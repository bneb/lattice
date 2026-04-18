#import <Foundation/Foundation.h>
#import <Network/Network.h>
#import <stdint.h>
#import <string.h>

// ============================================================================
// Epic 77: The TLS 1.3 ALPN Bridge (Phase 1)
// ============================================================================

nw_connection_t secure_connection = NULL;

extern void sys_net_send_h2_preface(void);
extern void sys_net_push_h2_bytes(const uint8_t *data, uint32_t len);

void sys_net_init_h2_connection(const char *hostname) {
  nw_endpoint_t endpoint = nw_endpoint_create_host(hostname, "443");

  nw_parameters_configure_protocol_block_t configure_tls =
      ^(nw_protocol_options_t tls_options) {
        sec_protocol_options_t sec_options =
            nw_tls_copy_sec_protocol_options(tls_options);
        sec_protocol_options_add_tls_application_protocol(sec_options, "h2");
        // Ensure TLS 1.3 is preferred
        sec_protocol_options_set_min_tls_protocol_version(
            sec_options, tls_protocol_version_TLSv13);
      };

  nw_parameters_t parameters = nw_parameters_create_secure_tcp(
      configure_tls, NW_PARAMETERS_DEFAULT_CONFIGURATION);

  secure_connection = nw_connection_create(endpoint, parameters);

  nw_connection_set_queue(
      secure_connection,
      dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0));

  nw_connection_set_state_changed_handler(
      secure_connection, ^(nw_connection_state_t state, nw_error_t error) {
        if (state == nw_connection_state_ready) {
          NSLog(@"[TLS] Connection Ready with h2 ALPN. Sending preface...");
          sys_net_send_h2_preface();

          // Start recursive read loop
          void (^__block receive_loop)(void);
          receive_loop = ^{
            nw_connection_receive(
                secure_connection, 1, 65536,
                ^(dispatch_data_t content, nw_content_context_t context,
                  bool is_complete, nw_error_t error) {
                  if (content) {
                    dispatch_data_apply(
                        content, ^bool(dispatch_data_t region, size_t offset,
                                       const void *buffer, size_t size) {
                          sys_net_push_h2_bytes((const uint8_t *)buffer,
                                                (uint32_t)size);
                          return true;
                        });
                  }
                  if (error == NULL && !is_complete) {
                    receive_loop();
                  }
                });
          };
          receive_loop();
        } else if (state == nw_connection_state_failed) {
          NSLog(@"[TLS] Connection Failed: %@", error);
        }
      });

  nw_connection_start(secure_connection);
}

void ext_tls_write_bytes(const uint8_t *data, uint32_t len) {
  if (!secure_connection)
    return;

  dispatch_data_t dispatch_data = dispatch_data_create(
      data, len, dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0),
      DISPATCH_DATA_DESTRUCTOR_DEFAULT);
  nw_connection_send(secure_connection, dispatch_data,
                     NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true,
                     ^(nw_error_t _Nullable error) {
                       if (error) {
                         NSLog(@"[TLS] Send Failed: %@", error);
                       }
                     });
}

extern void ext_ws_push_bytes(uint32_t socket_id, const uint8_t *data,
                              uint32_t len);

void sys_tls_start_ws_streaming_loop(uint32_t socket_id, nw_connection_t conn) {
  void (^__block receive_loop)(void);
  receive_loop = ^{
    nw_connection_receive(
        conn, 1, 65536,
        ^(dispatch_data_t content, nw_content_context_t context,
          bool is_complete, nw_error_t error) {
          if (content) {
            dispatch_data_apply(
                content, ^bool(dispatch_data_t region, size_t offset,
                               const void *buffer, size_t size) {
                  ext_ws_push_bytes(socket_id, (const uint8_t *)buffer,
                                    (uint32_t)size);
                  return true;
                });
          }
          if (error == NULL && !is_complete) {
            receive_loop();
          }
        });
  };
  receive_loop();
}

void sys_tls_upgrade_to_websocket(uint32_t socket_id, const char *path,
                                  const char *ws_key) {
  char upgrade_req[512];
  snprintf(upgrade_req, sizeof(upgrade_req),
           "GET %s HTTP/1.1\r\n"
           "Host: localhost\r\n"
           "Upgrade: websocket\r\n"
           "Connection: Upgrade\r\n"
           "Sec-WebSocket-Version: 13\r\n"
           "Sec-WebSocket-Key: %s\r\n\r\n",
           path, ws_key);

  dispatch_data_t data = dispatch_data_create(
      upgrade_req, strlen(upgrade_req),
      dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0),
      DISPATCH_DATA_DESTRUCTOR_DEFAULT);

  nw_connection_send(
      secure_connection, data, NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true,
      ^(nw_error_t error) {
        if (!error) {
          // Hijack the read loop to pump bytes directly into the WebSocket Ring
          // Buffer
          sys_tls_start_ws_streaming_loop(socket_id, secure_connection);
        }
      });
}
