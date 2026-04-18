// user/browser/audio_output.m — CoreAudio Hardware Bridge
// Epic 73: The Audio Matrix

#import <AudioToolbox/AudioToolbox.h>
#include <stdint.h>
#include <stdio.h>

// Salt Globals (Mangled Names)
extern float user__browser__media__AUDIO_RING_BUFFER[1048576];
extern uint32_t user__browser__media__AUDIO_HEAD;
extern uint32_t user__browser__media__AUDIO_TAIL;
extern uint64_t user__browser__media__AUDIO_SAMPLES_PLAYED;

static AudioComponentInstance outputAudioUnit;

/**
 * Real-Time Render Callback
 * Fires inside Apple's high-priority audio thread.
 * Drains the Salt AUDIO_RING_BUFFER and advances the Master Clock.
 */
static OSStatus renderAudioCallback(void *inRefCon, 
                                    AudioUnitRenderActionFlags *ioActionFlags, 
                                    const AudioTimeStamp *inTimeStamp, 
                                    UInt32 inBusNumber, 
                                    UInt32 inNumberFrames, 
                                    AudioBufferList *ioData) {
    
    // ASBD is configured for 32-bit Float Mono
    float *outBuffer = (float *)ioData->mBuffers[0].mData;
    
    // Local copies of atomic-ish pointers
    uint32_t head = user__browser__media__AUDIO_HEAD;
    uint32_t tail = user__browser__media__AUDIO_TAIL;
    
    for (UInt32 i = 0; i < inNumberFrames; i++) {
        if (head < tail) {
            // Pull sample from Float32 PCM Ring Buffer
            outBuffer[i] = user__browser__media__AUDIO_RING_BUFFER[head % 1048576];
            head++;
        } else {
            // Buffer underrun — fallback to silence to avoid DC offset pop
            outBuffer[i] = 0.0f; 
        }
    }
    
    // Commit indices back to Salt globals
    user__browser__media__AUDIO_HEAD = head;
    user__browser__media__AUDIO_SAMPLES_PLAYED += inNumberFrames;
    
    return noErr;
}

/**
 * Initializes the default CoreAudio output device.
 * Configures the pipeline for 44.1kHz synchronization.
 */
void sys_hw_audio_init() {
    AudioComponentDescription desc;
    desc.componentType = kAudioUnitType_Output;
    desc.componentSubType = kAudioUnitSubType_DefaultOutput;
    desc.componentFlags = 0;
    desc.componentFlagsMask = 0;
    desc.componentManufacturer = kAudioUnitManufacturer_Apple;
    
    AudioComponent inputComponent = AudioComponentFindNext(NULL, &desc);
    if (!inputComponent) {
        fprintf(stderr, "[Audio] FATAL: Failed to find default output component\n");
        return;
    }
    
    OSStatus status = AudioComponentInstanceNew(inputComponent, &outputAudioUnit);
    if (status != noErr) {
        fprintf(stderr, "[Audio] FATAL: Failed to create AudioUnit: %d\n", (int)status);
        return;
    }
    
    // AudioStreamBasicDescription: 32-bit Float Mono @ 44.1kHz
    // This is the target "Master Clock" configuration.
    AudioStreamBasicDescription asbd;
    asbd.mSampleRate = 44100.0;
    asbd.mFormatID = kAudioFormatLinearPCM;
    asbd.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved;
    asbd.mBitsPerChannel = 32;
    asbd.mChannelsPerFrame = 1;
    asbd.mFramesPerPacket = 1;
    asbd.mBytesPerFrame = 4;
    asbd.mBytesPerPacket = 4;
    asbd.mReserved = 0;
    
    status = AudioUnitSetProperty(outputAudioUnit, 
                                  kAudioUnitProperty_StreamFormat, 
                                  kAudioUnitScope_Input, 
                                  0, 
                                  &asbd, 
                                  sizeof(asbd));
                                  
    if (status != noErr) {
        fprintf(stderr, "[Audio] FATAL: Failed to set ASBD: %d\n", (int)status);
        return;
    }
    
    AURenderCallbackStruct callbackStruct;
    callbackStruct.inputProc = renderAudioCallback;
    callbackStruct.inputProcRefCon = NULL;
    
    status = AudioUnitSetProperty(outputAudioUnit, 
                                  kAudioUnitProperty_SetRenderCallback, 
                                  kAudioUnitScope_Input, 
                                  0, 
                                  &callbackStruct, 
                                  sizeof(callbackStruct));
                                  
    if (status != noErr) {
        fprintf(stderr, "[Audio] FATAL: Failed to set render callback: %d\n", (int)status);
        return;
    }
    
    AudioUnitInitialize(outputAudioUnit);
    AudioOutputUnitStart(outputAudioUnit);
    
    fprintf(stderr, "[Audio] Success: CoreAudio initialized at 44.1kHz Mono (32-bit Float)\n");
}
