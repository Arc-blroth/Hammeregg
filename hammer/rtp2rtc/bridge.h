#pragma once
#include <stdint.h>

typedef void (*hammer_rtp2rtc_ports_callback)(uint16_t video, uint16_t audio, void* user_data);
typedef void (*hammer_rtp2rtc_input_callback)();

void HammerRTP2RTCPortsCallbackBridge(
    hammer_rtp2rtc_ports_callback callback,
    uint16_t video,
    uint16_t audio,
    void* user_data);

void HammerRTP2RTCInputCallbackBridge(hammer_rtp2rtc_input_callback callback);
