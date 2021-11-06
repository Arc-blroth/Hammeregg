import * as BSON from "bson"
import { isIP } from "range_check"
import { StateMachine, StateMachineInstance } from "ts-state-machines"
import * as core from "./hammeregg_core"

/** jquery is dead, long live jquery! */
let $ = (id: string) => document.getElementById(id)

$("setup").onsubmit = e => {
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
    if(errors.length != 0) {
        setError(`Error${errors.length == 1 ? "" : "s"}: ${errors.join(", ")}!`)
        return
    } else {
        // clear the error
        setError()
    }

    // Init signalling
    try {
        initSignallingConnection(desktopName, signallingAddr)
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

function initSignallingConnection(desktopName: string, signallingAddr: string) {
    const SignallingStateMachine = StateMachine({
        initialState: "init",
        states: {
            init: { next: "waitRemoteInitResponse" },
            waitRemoteInitResponse: { next: "waitRemoteOfferResponse" },
            waitRemoteOfferResponse: { next: "waitRemoteOfferResponse" },
        },
    } as const)
    let state: StateMachineInstance<typeof SignallingStateMachine.config> = SignallingStateMachine()

    let connection = new WebSocket("wss://" + signallingAddr)
    connection.binaryType = "arraybuffer"
    connection.onopen = e => {
        let initPacket = BSON.serialize(<core.HandshakeInitPacket> {
            magic: core.MAGIC,
            version: core.VERSION_1_0,
            packet: <core.RemoteInitHandshakePacket> {
                type: core.HandshakePacketType.REMOTE_INIT,
                home_name: desktopName,
            }
        })
        connection.send(initPacket)
        state = state.next()
    }
    connection.onmessage = e => {
        try {
            let packet = BSON.deserialize(new Uint8Array(e.data)) as core.HandshakePacket
            console.log("Recieved packet:", packet)
            
            switch(packet.type) {
                case core.HandshakePacketType.REMOTE_INIT_RESPONSE: {
                    let initResponse = packet as core.RemoteInitResponseHandshakePacket
                    if(initResponse.response.hasOwnProperty("Err")) {
                        throw (initResponse.response as core.ResultErr).Err
                    } else {
                        connection.send(BSON.serialize(<core.RemoteOfferHandshakePacket> {
                            type: core.HandshakePacketType.REMOTE_OFFER,
                            peer: 0,
                            payload: [],
                        }))
                    }
                    break
                }
            }
        } catch(e) {
            console.error("Signalling error:", e)
            connection.close()
            setError("Signalling failed: " + e)
            ;($("setup") as HTMLFormElement).enabled = true
        }
    }
}