import * as BSON from "bson"
import { ASN1 } from "jsencrypt/lib/lib/asn1js/asn1"
import { Base64 as ASNBase64 } from "jsencrypt/lib/lib/asn1js/base64"
import { parseBigInt } from "jsencrypt/lib/lib/jsbn/jsbn"
import { JSEncrypt } from "jsencrypt"
import { JSEncryptRSAKey } from "jsencrypt/lib/JSEncryptRSAKey"
import { isIP } from "range_check"
import { StateMachine, StateMachineInstance } from "ts-state-machines"
import * as core from "./hammeregg_core"
import { RemotePassword } from "./hammeregg_key"

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

    let encryptor: JSEncrypt
    let decryptor: JSEncrypt
    let eggPasswordFiles = ($("egg_password") as HTMLInputElement).files
    if(eggPasswordFiles.length > 0) {
        try {
            let password = BSON.deserialize(await eggPasswordFiles[0].arrayBuffer()) as RemotePassword
            encryptor = new JSEncrypt()
            let parsedPublicKey = parsePublicKey(password.home_public_key)
            ;(encryptor as any).key = parsedPublicKey

            decryptor = new JSEncrypt()
            decryptor.setPrivateKey(password.remote_private_key)
        } catch(e) {
            console.error(e)
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
        initSignallingConnection(desktopName, signallingAddr, encryptor, decryptor)
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
 * Parses an ASN.1 DER-encoded key with the structure
 * ```
 * RSAPublicKey ::= SEQUENCE {
 *     modulus           INTEGER,  -- n
 *     publicExponent    INTEGER   -- e
 * }
 * ```
 */
function parsePublicKey(pem: string) {
    let asn1 = ASN1.decode(ASNBase64.unarmor(pem))
    let modulus = asn1.sub[0].getHexStringValue()
    let n = parseBigInt(modulus, 16)
    let publicExponent = asn1.sub[1].getHexStringValue()
    let e = parseInt(publicExponent, 16)

    // we do a bit of privacy breaking
    let rsaKey = new JSEncryptRSAKey()
    ;(rsaKey as any).n = n
    ;(rsaKey as any).e = e

    return rsaKey
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
    encryptor: JSEncrypt,
    decryptor: JSEncrypt,
) {
    const SignallingStateMachine = StateMachine({
        initialState: "init",
        states: {
            init: { next: "waitRemoteInitResponse" },
            waitRemoteInitResponse: { next: "waitRemoteOfferResponse" },
            waitRemoteOfferResponse: { next: "waitRemoteOfferResponse" },
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
    peerConnection.addTransceiver("audio", {"direction": "recvonly"})
    peerConnection.addTransceiver("video", {"direction": "recvonly"})
    peerConnection.createDataChannel("hammeregg-input", {"negotiated": false})
    let allCandidatesGathered = new Promise<RTCSessionDescription>((resolve, _) => {
        peerConnection.onicecandidate = e => {
            if(e.candidate === null) {
                resolve(peerConnection.localDescription)
            }
        }
    })
    peerConnection.createOffer().then(d => peerConnection.setLocalDescription(d))

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
                        let localSessionDescription = await allCandidatesGathered
                        let encryptedLocalSD = encryptor.encrypt(localSessionDescription.sdp)
                        if(encryptedLocalSD == false) throw "couldn't encrypt session description"
                        signallingConnection.send(BSON.serialize(<core.RemoteOfferHandshakePacket> {
                            type: core.HandshakePacketType.REMOTE_OFFER,
                            peer: 0, // this is filled in by Rooster
                            payload: Array.from(atob(encryptedLocalSD), x => x.charCodeAt(0)),
                        }))
                        state = state.next()
                    }
                    break
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