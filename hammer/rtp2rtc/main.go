package main

// #include <stdlib.h>
// #include <stdint.h>
// #include <bridge.h>
import "C"
import (
    "encoding/json"
    "fmt"
    "runtime/cgo"
    "unsafe"

    "github.com/pion/webrtc/v3"
)

// Since rtp2rtc is always built as a
// static library this is a no-op.
func main() {}

// A null `uintptr_t`, for use with FFI. 
const Nullptr = C.uintptr_t(0)

type PeerConnection struct {
    Connection *webrtc.PeerConnection
    VideoSender *webrtc.RTPSender
    AudioSender *webrtc.RTPSender
    InputChannel *webrtc.DataChannel
}

//export hammer_rtp2rtc_init
func hammer_rtp2rtc_init() C.uintptr_t {
    fmt.Println("[Hammer/Pion] init()")

    connection, err := webrtc.NewPeerConnection(webrtc.Configuration{
        ICEServers: []webrtc.ICEServer{
            {
                URLs: []string{"stun:stun3.l.google.com:19302"},
            },
        },
    })
    if err != nil {
        return Nullptr
    }

    // video track
    videoTrack, err := webrtc.NewTrackLocalStaticRTP(webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypeVP8}, "video", "hammeregg-stream")
    if err != nil {
        return Nullptr
    }
    videoSender, err := connection.AddTrack(videoTrack)
    if err != nil {
        return Nullptr
    }

    // audio track
    audioTrack, err := webrtc.NewTrackLocalStaticRTP(webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypeOpus}, "audio", "hammeregg-stream")
    if err != nil {
        return Nullptr
    }
    audioSender, err := connection.AddTrack(audioTrack)
    if err != nil {
        return Nullptr
    }

    // inputs channel
    noNegotiation := false
    inputChannel, err := connection.CreateDataChannel("hammeregg-input", &webrtc.DataChannelInit{Negotiated: &noNegotiation})
    if err != nil {
        return Nullptr
    }

    peerConnection := PeerConnection {
        Connection: connection,
        VideoSender: videoSender,
        AudioSender: audioSender,
        InputChannel: inputChannel,
    }

    return C.uintptr_t(cgo.NewHandle(peerConnection))
}

//export hammer_rtp2rtc_build_offer
func hammer_rtp2rtc_build_offer(offerPtr *C.char) C.uintptr_t {
    offerBytes := []byte(C.GoString(offerPtr))
    desc := webrtc.SessionDescription{}

    // deserialize from json
    if err := json.Unmarshal(offerBytes, &desc); err != nil {
        return Nullptr
    }

    // validate session description
    if _, err := desc.Unmarshal(); err != nil {
        return Nullptr
    }

    return C.uintptr_t(cgo.NewHandle(desc))
}

//export hammer_rtp2rtc_signal_offer
func hammer_rtp2rtc_signal_offer(connection C.uintptr_t, descPtr C.uintptr_t) *C.char {
    peerConnection := cgo.Handle(connection).Value().(PeerConnection)
    desc := cgo.Handle(descPtr).Value().(webrtc.SessionDescription)

    // set remote SessionDescription
    if err := peerConnection.Connection.SetRemoteDescription(desc); err != nil {
        return nil
    }

    // set local SessionDescription
    answer, err := peerConnection.Connection.CreateAnswer(nil)
    if err != nil {
        return nil
    }
    if err = peerConnection.Connection.SetLocalDescription(answer); err != nil {
        return nil
    }

    // wait for ICE
    <-webrtc.GatheringCompletePromise(peerConnection.Connection)

    // return answer
    outAnswer, err := json.Marshal(peerConnection.Connection.LocalDescription())
    if err != nil {
        return nil
    }
    return C.CString(string(outAnswer))
}

//export hammer_rtp2rtc_start
func hammer_rtp2rtc_start(connection C.uintptr_t, port C.uint16_t, callback C.hammer_rtp2rtc_input_callback) {
    C.HammerRTP2RTCInputCallbackBridge(callback)
}

//export hammer_rtp2rtc_stop
func hammer_rtp2rtc_stop(connection C.uintptr_t) {
    cgo.Handle(connection).Delete()
}

//export hammer_free_cstring
func hammer_free_cstring(cstring *C.char) {
    C.free(unsafe.Pointer(cstring))
}
