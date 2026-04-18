#import <VideoToolbox/VideoToolbox.h>
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>

extern uint8_t user__browser__media__MEDIA_RING_BUFFER[33554432];
extern uint32_t user__browser__media__MEDIA_HEAD;
extern uint32_t user__browser__media__MEDIA_TAIL;

VTDecompressionSessionRef decoderSession = NULL;
CMVideoFormatDescriptionRef format_desc = NULL;

CVPixelBufferRef ready_frames[16];
uint32_t frame_head = 0;
uint32_t frame_tail = 0;

uint8_t cached_sps[512];
size_t sps_len = 0;

uint8_t cached_pps[512];
size_t pps_len = 0;

void decompressionOutputCallback(
    void *decompressionOutputRefCon, 
    void *sourceFrameRefCon, 
    OSStatus status, 
    VTDecodeInfoFlags infoFlags, 
    CVImageBufferRef imageBuffer, 
    CMTime presentationTimeStamp, 
    CMTime presentationDuration) 
{
    if (status != noErr || !imageBuffer) {
        printf("[Decoder] Error or null imagebuffer in callback\n");
        return;
    }
    
    // Retain the frame and push to the ready queue for the Paint engine
    CVBufferRetain(imageBuffer);
    ready_frames[frame_tail % 16] = imageBuffer;
    frame_tail++;
    printf("[Decoder] Frame decoded synchronously by hardware! frame_tail = %u. IOSurface: %d\n", frame_tail, CVPixelBufferGetIOSurface(imageBuffer) != NULL);
}

void sys_media_process_nal_unit(uint8_t* nal_data, size_t nal_len, uint8_t nal_type) {
    if (nal_type == 7) { // Set SPS
        if (nal_len < sizeof(cached_sps)) {
            memcpy(cached_sps, nal_data, nal_len);
            sps_len = nal_len;
        }
    } else if (nal_type == 8) { // Set PPS
        if (nal_len < sizeof(cached_pps)) {
            memcpy(cached_pps, nal_data, nal_len);
            pps_len = nal_len;
        }
    }

    if (sps_len > 0 && pps_len > 0 && !format_desc) {
        const uint8_t* parameterSetPointers[2] = { cached_sps, cached_pps };
        const size_t parameterSetSizes[2] = { sps_len, pps_len };
        OSStatus status = CMVideoFormatDescriptionCreateFromH264ParameterSets(kCFAllocatorDefault, 2, parameterSetPointers, parameterSetSizes, 4, &format_desc);
        
        if (status == noErr) {
            NSMutableDictionary *destinationPixelBufferAttributes = [NSMutableDictionary dictionary];
            [destinationPixelBufferAttributes setObject:[NSNumber numberWithInt:kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange] forKey:(id)kCVPixelBufferPixelFormatTypeKey];
            [destinationPixelBufferAttributes setObject:@{} forKey:(id)kCVPixelBufferIOSurfacePropertiesKey];

            VTDecompressionOutputCallbackRecord callbackRecord = { decompressionOutputCallback, NULL };
            VTDecompressionSessionCreate(kCFAllocatorDefault, format_desc, NULL, (__bridge CFDictionaryRef)destinationPixelBufferAttributes, &callbackRecord, &decoderSession);
            printf("[Decoder] VTDecompressionSession initialized!\n");
        } else {
            printf("[Decoder] CMVideoFormatDescriptionCreateFromH264ParameterSets failed!\n");
        }
    }

    if ((nal_type == 1 || nal_type == 5) && decoderSession && format_desc) {
        // VideoToolbox requires AVCC format for decoding (4-byte length prefix instead of Annex B start codes)
        uint32_t avcc_len = htonl((uint32_t)nal_len);
        size_t block_len = nal_len + 4;
        
        CMBlockBufferRef blockBuffer = NULL;
        OSStatus bbStatus = CMBlockBufferCreateWithMemoryBlock(kCFAllocatorDefault, NULL, block_len, kCFAllocatorDefault, NULL, 0, block_len, kCMBlockBufferAssureMemoryNowFlag, &blockBuffer);
            
        if (bbStatus == kCMBlockBufferNoErr) {
            CMBlockBufferReplaceDataBytes(&avcc_len, blockBuffer, 0, 4);
            CMBlockBufferReplaceDataBytes(nal_data, blockBuffer, 4, nal_len);
            
            CMSampleBufferRef sampleBuffer = NULL;
            OSStatus sbStatus = CMSampleBufferCreate(kCFAllocatorDefault, blockBuffer, true, NULL, NULL, format_desc, 1, 0, NULL, 0, NULL, &sampleBuffer);
                
            if (sbStatus == noErr) {
                VTDecodeInfoFlags flagsOut;
                VTDecompressionSessionDecodeFrame(decoderSession, sampleBuffer, kVTDecodeFrame_EnableAsynchronousDecompression, NULL, &flagsOut);
                CFRelease(sampleBuffer);
            }
            CFRelease(blockBuffer);
        }
    }
}

void sys_hw_decoder_signal_data_ready() {
    uint32_t tail = user__browser__media__MEDIA_TAIL;
    uint8_t* ring = user__browser__media__MEDIA_RING_BUFFER;
    uint32_t ring_size = 33554432;
    
    while (user__browser__media__MEDIA_HEAD < tail) {
        uint32_t scan = user__browser__media__MEDIA_HEAD;
        uint32_t start_code_pos = 0xFFFFFFFF;
        uint32_t next_start_code_pos = 0xFFFFFFFF;
        
        // Find first start code
        for (uint32_t i = scan; i < tail - 3; i++) {
            if (ring[i % ring_size] == 0 && ring[(i+1) % ring_size] == 0 && ring[(i+2) % ring_size] == 0 && ring[(i+3) % ring_size] == 1) {
                start_code_pos = i;
                break;
            }
        }
        
        if (start_code_pos == 0xFFFFFFFF) {
            user__browser__media__MEDIA_HEAD = tail; // Consumed all non-NAL bytes
            return;
        }
        
        // Find next start code to bound this current NAL
        for (uint32_t i = start_code_pos + 4; i < tail - 3; i++) {
            if (ring[i % ring_size] == 0 && ring[(i+1) % ring_size] == 0 && ring[(i+2) % ring_size] == 0 && ring[(i+3) % ring_size] == 1) {
                next_start_code_pos = i;
                break;
            }
        }
        
        if (next_start_code_pos != 0xFFFFFFFF) {
            uint32_t nal_start = start_code_pos + 4;
            uint32_t nal_len = next_start_code_pos - nal_start;
            uint8_t nal_type = ring[nal_start % ring_size] & 0x1F;
            
            uint8_t* linear_nal = malloc(nal_len);
            for (uint32_t i = 0; i < nal_len; i++) {
                linear_nal[i] = ring[(nal_start + i) % ring_size];
            }
            
            sys_media_process_nal_unit(linear_nal, nal_len, nal_type);
            free(linear_nal);
            
            user__browser__media__MEDIA_HEAD = next_start_code_pos; // Advance head
        } else {
            // Not enough data for full NAL, leave HEAD pointing to start of NAL
            user__browser__media__MEDIA_HEAD = start_code_pos;
            return; 
        }
    }
}

CVPixelBufferRef get_latest_video_frame(void) {
    if (frame_tail > 0) {
        return ready_frames[(frame_tail - 1) % 16];
    }
    return NULL;
}
