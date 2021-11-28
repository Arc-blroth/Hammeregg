import * as BSON from "bson"
import { isIP } from "range_check"
import { StateMachine, StateMachineInstance } from "ts-state-machines"
import * as core from "./hammeregg_core"
import * as key from "./hammeregg_key"
import * as input from "./input"

/** jquery is dead, long live jquery! */
let $ = (id: string) => document.getElementById(id)

$("egg_password").onchange = e => {
    let path = ($("egg_password") as HTMLInputElement).value
    if(path) {
        // see https://html.spec.whatwg.org/multipage/input.html#fakepath-srsly
        let displayPath
        if (path.substring(0, 12).toLowerCase() == "c:\\fakepath\\") {
            displayPath = path.substring(12)
        } else {
            path = path.replace(/\\/g, "/")
            let lastIndexOfSlash = path.lastIndexOf("/")
            if(lastIndexOfSlash > 0) {
                displayPath = path.substring(lastIndexOfSlash + 1)
            } else {
                displayPath = path
            }
        }
        $("egg-password-display").innerText = displayPath
    } else {
        $("egg-password-display").innerText = "No file selected"
    }
}

$("fake_egg_password").onclick = e => {
    e.preventDefault()
    $("egg_password").click()
}

$("setup").onsubmit = async e => {
    e.preventDefault()
    ;($("setup") as HTMLFormElement).enabled = false

    let desktopName = ($("desktop_name") as HTMLInputElement).value
    let signallingAddr = ($("signalling_server_ip") as HTMLInputElement).value

    // Validate
    let errors = []

    if(desktopName.length == 0) {
        errors.push("desktop name cannot be empty")
    } else if(desktopName.includes("\0")) {
        errors.push("desktop name cannot contain '\\0'")
    }

    signallingAddr = tryParseSignallingServerAddr(signallingAddr)
    if(!signallingAddr) {
        errors.push("signalling server is not a valid ip:port")
    }

    let homePublicKey: CryptoKey
    let remotePrivateKey: CryptoKey
    let eggPasswordFiles = ($("egg_password") as HTMLInputElement).files
    if(eggPasswordFiles.length > 0) {
        try {
            let password = BSON.deserialize(await eggPasswordFiles[0].arrayBuffer()) as key.RemotePassword
            homePublicKey = await key.importRSAPublicKey(password.home_public_key)
            remotePrivateKey = await key.importRSAPrivateKey(password.remote_private_key)
        } catch(e) {
            if(e instanceof DOMException) {
                console.error(`${e.name}: ${e.message}`)
            } else {
                console.error(e)
            }
            errors.push("couldn't read password")
        }
    } else {
        errors.push("no password file selected")
    }

    if(errors.length != 0) {
        setError(`Error${errors.length == 1 ? "" : "s"}: ${errors.join(", ")}!`)
        return
    } else {
        // clear the error
        setError()
    }

    // Init signalling
    try {
        initSignallingConnection(desktopName, signallingAddr, homePublicKey, remotePrivateKey)
    } catch {
        ($("setup") as HTMLFormElement).enabled = true
    }
}

/**
 * Parses the given `ip[:port]` into a valid socket address,
 * adding the default Hammeregg signalling port if necessary.
 * Returns the parsed address on success and `null` on error.
 */
function tryParseSignallingServerAddr(ipAndPort: string): string {
    let split = ipAndPort.split(":")
    if(split.length > 2) return null

    let ip = split[0]
    if(!isIP(ip)) return null

    let port = split.length == 2 ? split[1] : core.DEFAULT_HAMMEREGG_PORT
    if(port < 0 || port >= 1 << 16) return null

    return `${ip}:${port}`
}

/**
 * Sets the setup dialog's error text to the given error message,
 * or clears the error message if no argument is given.
 */
function setError(error?: string) {
    $("setup_err").textContent = error ? error : "\u00a0"
}

function initSignallingConnection(
    desktopName: string,
    signallingAddr: string,
    homePublicKey: CryptoKey,
    remotePrivateKey: CryptoKey,
) {
    const SignallingStateMachine = StateMachine({
        initialState: "init",
        states: {
            init: { next: "waitRemoteInitResponse" },
            waitRemoteInitResponse: { next: "waitHomeAnswerResponse" },
            waitHomeAnswerResponse: { next: "done" },
            done: { next: "done" },
        },
    } as const)

    let state: StateMachineInstance<typeof SignallingStateMachine.config> = SignallingStateMachine()
    let assertState = stateName => {
        if(state.state != stateName) throw `not in state ${stateName}`
    }

    // init the peer connection we're going to be setting up
    let peerConnection = new RTCPeerConnection({
        iceServers: [{ urls: 'stun:stun3.l.google.com:19302' }]
    })

    peerConnection.ontrack = e => {
        console.log(e)
        if(e.track.kind == "video") {
            ($("stream-video") as HTMLVideoElement).srcObject = e.streams[0]
        }
    }

    peerConnection.addTransceiver("audio", {"direction": "recvonly"})
    peerConnection.addTransceiver("video", {"direction": "recvonly"})
    let inputChannel = peerConnection.createDataChannel("hammeregg-input", {"negotiated": false})
    let allCandidatesGathered = new Promise<RTCSessionDescription>((resolve, _) => {
        peerConnection.onicecandidate = e => {
            if(e.candidate === null) {
                resolve(peerConnection.localDescription)
            }
        }
    })
    peerConnection.createOffer().then(d => peerConnection.setLocalDescription(d))

    // init signalling connection
    let signallingConnection = new WebSocket("wss://" + signallingAddr)
    signallingConnection.binaryType = "arraybuffer"
    signallingConnection.onopen = e => {
        let initPacket = BSON.serialize(<core.HandshakeInitPacket> {
            magic: core.MAGIC,
            version: core.VERSION_1_0,
            packet: <core.RemoteInitHandshakePacket> {
                type: core.HandshakePacketType.REMOTE_INIT,
                home_name: desktopName,
            }
        })
        signallingConnection.send(initPacket)
        state = state.next()
    }

    signallingConnection.onmessage = async e => {
        try {
            let packet = BSON.deserialize(new Uint8Array(e.data)) as core.HandshakePacket
            console.log("Recieved packet:", packet)
            
            switch(packet.type) {
                case core.HandshakePacketType.REMOTE_INIT_RESPONSE: {
                    assertState("waitRemoteInitResponse")
                    let initResponse = packet as core.RemoteInitResponseHandshakePacket
                    if(initResponse.response.hasOwnProperty("Err")) {
                        throw (initResponse.response as core.ResultErr).Err
                    } else {
                        // wait for our session description
                        let localSessionDescription = JSON.stringify(await allCandidatesGathered)

                        // generate a random aes key and encrypt our payload
                        let aesKey = await key.generateAESKey()
                        let aesIV = key.generateIV()
                        let encryptedLocalSD = await crypto.subtle.encrypt(
                            { name: "AES-GCM", iv: aesIV },
                            aesKey,
                            key.string2Buffer(localSessionDescription)
                        ) as ArrayBuffer
                        let exportedKey = await crypto.subtle.wrapKey("raw", aesKey, homePublicKey, key.HAMMEREGG_RSA_PARAMS)
                        
                        let out: core.RemoteOfferHandshakePacket = {
                            type: core.HandshakePacketType.REMOTE_OFFER,
                            peer: 0, // this is filled in by Rooster
                            key: key.buffer2Array(exportedKey),
                            iv: key.buffer2Array(aesIV),
                            payload: key.buffer2Array(encryptedLocalSD),
                        };
                        console.log("Sending remote offer ", out)
                        signallingConnection.send(BSON.serialize(out))
                        state = state.next()
                    }
                    break
                }
                case core.HandshakePacketType.HOME_ANSWER_SUCCESS: {
                    assertState("waitHomeAnswerResponse")
                    let answer = packet as core.HomeAnswerSuccessHandshakePacket

                    // decrypt payload
                    let aesKey = await crypto.subtle.unwrapKey(
                        "raw",
                        key.array2Buffer(answer.key),
                        remotePrivateKey,
                        key.HAMMEREGG_RSA_PARAMS,
                        key.HAMMEREGG_AES_PARAMS,
                        false,
                        ["decrypt"]
                    )
                    let aesIV = key.array2Buffer(answer.iv)
                    let decryptedAnswer = await crypto.subtle.decrypt(
                        { name: "AES-GCM", iv: aesIV },
                        aesKey,
                        key.array2Buffer(answer.payload)
                    ) as ArrayBuffer

                    let remoteSessionDescription =JSON.parse(key.buffer2String(decryptedAnswer))
                    console.log("Received remote description ", remoteSessionDescription)

                    // set remote description!
                    peerConnection.setRemoteDescription(new RTCSessionDescription(remoteSessionDescription))
                    state = state.next()
                    signallingConnection.close()

                    // show the actual desktop
                    showStream(inputChannel)
                    break
                }
                case core.HandshakePacketType.HOME_ANSWER_FAILURE: {
                    assertState("waitHomeAnswerResponse")
                    let answer = packet as core.HomeAnswerFailureHandshakePacket
                    throw answer.error
                }
            }
        } catch(e) {
            console.error("Signalling error:", e)
            signallingConnection.close()
            setError("Signalling failed: " + e)
            ;($("setup") as HTMLFormElement).enabled = true
        }
    }
}

function showStream(inputChannel: RTCDataChannel) {
    $("setup-wrapper").classList.add("hidden")
    $("stream-wrapper").classList.remove("hidden")
    
    let streamVideo = $("stream-video") as HTMLVideoElement
    streamVideo.autoplay = true
    streamVideo.controls = false
    input.setup(inputChannel, streamVideo)
}