#pragma once

typedef void (*hammer_rtp2rtc_input_callback)();

void HammerRTP2RTCInputCallbackBridge(hammer_rtp2rtc_input_callback callback);
