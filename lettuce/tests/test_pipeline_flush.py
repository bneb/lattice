import socket

def test_pipeline_flush():
    # Connect to lettuce
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect(('127.0.0.1', 6379))
    
    # send_buf_cap is 4096. Each PING response is +PONG\r\n (7 bytes).
    # We need > 4096 bytes of responses. 4096 / 7 = 585.1
    # If we send 600 PINGs, it will exceed 4096 bytes, triggering the flush.
    
    pipeline = b"*1\r\n$4\r\nPING\r\n" * 600
    s.sendall(pipeline)
    
    # Receive 600 responses
    received = 0
    expected = 600 * 7
    while received < expected:
        data = s.recv(4096)
        if not data:
            break
        received += len(data)
        
    s.close()
    
    if received == expected:
        print("PASS: pipeline fallback flush")
    else:
        print(f"FAIL: expected {expected} bytes, got {received}")
        exit(1)

if __name__ == "__main__":
    test_pipeline_flush()
