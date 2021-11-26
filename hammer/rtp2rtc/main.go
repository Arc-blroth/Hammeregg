package main

// #include <stdlib.h>
// #include <stdint.h>
// #include <bridge.h>
import "C"
import (
    "encoding/json"
    "errors"
    "fmt"
    "io"
    "net"
    "os"
    "runtime/cgo"
    "unsafe"

    "github.com/pion/webrtc/v3"
)

// Since rtp2rtc is always built as a
// static library this is a no-op.
func main() {}

// A null `uintptr_t`, for use with FFI. 
const Nullptr = C.uintptr_t(0)

// Buffer size for all IO connections.
const NetBufferSize = 1024;

func LogInfo(format string, args ...interface{}) {
    fmt.Printf("[Hammer/Pion] %s\n", fmt.Sprintf(format, args...))
}

// Why does Go not have a fmt.Eprintf
func LogError(format string, args ...interface{}) {
    fmt.Fprintf(os.Stderr, "[Hammer/Pion] %s\n", fmt.Sprintf(format, args...))
}

type PeerConnection struct {
    Connection *webrtc.PeerConnection
    VideoTrack *webrtc.TrackLocalStaticRTP
    VideoSender *webrtc.RTPSender
    AudioTrack *webrtc.TrackLocalStaticRTP
    AudioSender *webrtc.RTPSender
    InputChannel *webrtc.DataChannel
    StopNotifier *chan struct{}
}

//export hammer_rtp2rtc_init
func hammer_rtp2rtc_init() C.uintptr_t {
    LogInfo("init()")

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

    stopNotifier := make(chan struct{})

    peerConnection := PeerConnection {
        Connection: connection,
        VideoTrack: videoTrack,
        VideoSender: videoSender,
        AudioTrack: audioTrack,
        AudioSender: audioSender,
        InputChannel: inputChannel,
        StopNotifier: &stopNotifier,
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
func hammer_rtp2rtc_start(
    connection C.uintptr_t,
    ports_callback C.hammer_rtp2rtc_ports_callback,
    ports_callback_user_data unsafe.Pointer,
    input_callback C.hammer_rtp2rtc_input_callback,
) {
    LogInfo("start()")
    peerConnection := cgo.Handle(connection).Value().(PeerConnection)

    defer func() {
        // Make sure to close the peer connection before returning
        if err := peerConnection.Connection.Close(); err != nil {
            LogError("Couldn't close peer connection: %s", err)
            panic(err)
        }
    }()

    // Read video and audio packets from remote for ACKs
    go func() {
        buffer := make([]byte, NetBufferSize)
        for {
            if _, _, err := peerConnection.AudioSender.Read(buffer); err != nil {
                if(!errors.Is(err, io.ErrClosedPipe)) {
                    LogError("Couldn't read from audio sender: %s", err)
                }
                return
            }
        }
    }()
    go func() {
        buffer := make([]byte, NetBufferSize)
        for {
            if _, _, err := peerConnection.VideoSender.Read(buffer); err != nil {
                if(!errors.Is(err, io.ErrClosedPipe)) {
                    LogError("Couldn't read from video sender: %s", err)
                }
                return
            }
        }
    }()

    // Open UDP listeners on the given ports
    videoListener, err := net.ListenUDP("udp", &net.UDPAddr{IP: net.ParseIP("127.0.0.1")})
    videoPort := videoListener.LocalAddr().(*net.UDPAddr).Port
    if err != nil {
        LogError("Binding to video port %d failed!: %s", videoPort, err)
        panic(err)
    }
    audioListener, err := net.ListenUDP("udp", &net.UDPAddr{IP: net.ParseIP("127.0.0.1")})
    audioPort := audioListener.LocalAddr().(*net.UDPAddr).Port
    if err != nil {
        LogError("Binding to audio port %d failed!: %s", audioPort, err)
        panic(err)
    }
    
    defer func() {
        // Make sure to close the UDP listeners before returning
        if err = videoListener.Close(); err != nil {
            LogError("Couldn't close video listener: %s", err)
            panic(err)
        }
        if err = audioListener.Close(); err != nil {
            LogError("Couldn't close audio listener: %s", err)
            panic(err)
        }
    }()

    // give the bound ports back to caller
    C.HammerRTP2RTCPortsCallbackBridge(
        ports_callback,
        C.uint16_t(videoPort),
        C.uint16_t(audioPort),
        ports_callback_user_data,
    )
    
    // Read packets from local ports and forward them to the remote
    go func() {
        buffer := make([]byte, NetBufferSize * 2)
        for {
            n, _, err := videoListener.ReadFrom(buffer)
            if err != nil {
                if(errors.Is(err, net.ErrClosed)) {
                    // graceful shutdown
                    return
                } else {
                    LogError("Couldn't read from video port %d: %s", videoPort, err)
                    panic(err)
                }
            }

            if _, err = peerConnection.VideoTrack.Write(buffer[:n]); err != nil && !errors.Is(err, io.ErrClosedPipe) {
                LogError("Couldn't write to video track: %s", err)
                panic(err)
            }
        }
    }()
    go func() {
        buffer := make([]byte, NetBufferSize * 2)
        for {
            n, _, err := audioListener.ReadFrom(buffer)
            if err != nil {
                if(errors.Is(err, net.ErrClosed)) {
                    // graceful shutdown
                    return
                } else {
                    LogError("Couldn't read from audio port %d: %s", audioPort, err)
                    panic(err)
                }
            }

            if _, err = peerConnection.AudioTrack.Write(buffer[:n]); err != nil && !errors.Is(err, io.ErrClosedPipe) {
                LogError("Couldn't write to audio track: %s", err)
                panic(err)
            }
        }
    }()

    // Wait for the stop notifier to be called
    <-*peerConnection.StopNotifier
}

//export hammer_rtp2rtc_stop
func hammer_rtp2rtc_stop(connection C.uintptr_t) {
    peerConnection := cgo.Handle(connection).Value().(PeerConnection)
    *peerConnection.StopNotifier <- struct{}{}
}

//export hammer_rtp2rtc_free
func hammer_rtp2rtc_free(connection C.uintptr_t) {
    cgo.Handle(connection).Delete()
}

//export hammer_free_cstring
func hammer_free_cstring(cstring *C.char) {
    C.free(unsafe.Pointer(cstring))
}
