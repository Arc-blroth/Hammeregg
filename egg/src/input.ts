import * as BSON from "bson"
import { Input, KeyInput, MouseButton, SpecialKeyInput } from "./hammeregg_core"

/**
 * Sets up sending input to a home computer.
 */
export function setup(channel: RTCDataChannel, video: HTMLVideoElement) {
    let keyHandler = (e: KeyboardEvent, ty: string) => {
        if (!e.isComposing) {
            let keyInput = keyEventToKeyInput(e)
            if (keyInput !== null) {
                channel.send(BSON.serialize({ [ty]: keyInput }))
            }
        }
    }
    video.onkeydown = e => keyHandler(e, "key_down")
    video.onkeyup = e => keyHandler(e, "key_up")

    let buttHandler = (e: MouseEvent, ty: string) => {
        let button: MouseButton
        switch (e.button) {
            case 0:
                button = MouseButton.Left
                break
            case 1:
                button = MouseButton.Middle
                break
            case 2:
                button = MouseButton.Right
                break
            default:
                return
        }
        channel.send(BSON.serialize({ [ty]: button }))
    }
    video.onmousedown = e => buttHandler(e, "mouse_down")
    video.onmouseup = e => buttHandler(e, "mouse_up")

    video.onwheel = e =>
        channel.send(
            BSON.serialize({ mouse_scroll: { x: e.clientX, y: e.clientY } })
        )

    video.onmousemove = e => {
        // calculate actual video bounds
        let windowRatio = window.innerHeight / window.innerWidth
        let videoRatio = video.videoHeight / video.videoWidth
        let minX: number
        let minY: number
        let scaledW: number
        let scaledH: number
        if (videoRatio >= windowRatio) {
            minX = (window.innerWidth - window.innerHeight / videoRatio) / 2
            minY = 0
            scaledW = window.innerWidth - minX * 2
            scaledH = window.innerHeight
        } else {
            minX = 0
            minY = (window.innerHeight - window.innerWidth * videoRatio) / 2
            scaledW = window.innerWidth
            scaledH = window.innerHeight - minY * 2
        }

        let x = (e.clientX - minX) / scaledW
        let y = (e.clientY - minY) / scaledH
        channel.send(BSON.serialize({ mouse_move: { x: x, y: y } }))
    }
}

// see https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/key/Key_Values
// for all key values
const BROWSER2ENIGO_SPECIAL_KEY_MAP = new Map(
    Object.keys(SpecialKeyInput)
        .filter(item => isNaN(Number(item)))
        .map(enigoKey => {
            let browserKey: string

            if (enigoKey.endsWith("Arrow")) {
                // Reverse the position of "Arrow" in arrow key strings
                browserKey =
                    "Arrow" + enigoKey.substring(0, enigoKey.length - 5)
            } else if (enigoKey === "Return") {
                browserKey = "Enter"
            } else if (enigoKey === "Space") {
                browserKey = " "
            } else {
                browserKey = enigoKey
            }

            return [browserKey, enigoKey as SpecialKeyInput]
        })
)

function keyEventToKeyInput(e: KeyboardEvent): KeyInput | null {
    if (e.key >= "!" && e.key <= "~") {
        return { alpha_key: e.key }
    } else if (BROWSER2ENIGO_SPECIAL_KEY_MAP.has(e.key)) {
        return { special_key: BROWSER2ENIGO_SPECIAL_KEY_MAP.get(e.key) }
    } else if (e.hasOwnProperty("keyCode")) {
        // @ts-ignore
        return { raw_key: e.keyCode }
    } else {
        return null
    }
}
