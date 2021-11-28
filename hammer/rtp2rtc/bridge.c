#include <bridge.h>

void HammerRTP2RTCPortsCallbackBridge(
    hammer_rtp2rtc_ports_callback callback,
    uint16_t video,
    uint16_t audio,
    void* user_data) {
    callback(video, audio, user_data);
}

void HammerRTP2RTCInputCallbackBridge(hammer_rtp2rtc_input_callback callback,
                                      void* input_packet,
                                      size_t input_packet_len,
                                      void* user_data) {
    callback(input_packet, input_packet_len, user_data);
}
