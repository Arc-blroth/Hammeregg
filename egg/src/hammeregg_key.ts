// Maintainers: keep this interface synchronized with
// `RemotePassword` in hammer/hammeregg_backend/src/key.rs
export interface RemotePassword {
    home_public_key: string;
    remote_private_key: string;
}

const PUBLIC_KEY_HEADER = "-----BEGIN PUBLIC KEY-----"
const PUBLIC_KEY_FOOTER = "-----END PUBLIC KEY-----"
const PRIVATE_KEY_HEADER = "-----BEGIN PRIVATE KEY-----"
const PRIVATE_KEY_FOOTER = "-----END PRIVATE KEY-----"
export const HAMMEREGG_RSA_PARAMS = { name: "RSA-OAEP", hash: "SHA-256" }
export const HAMMEREGG_AES_PARAMS = { name: "AES-GCM", length: 256 }

/**
 * Converts a string to a UTF-8 byte buffer.
 */
export function string2Buffer(str: string): ArrayBuffer {
    const buffer = new ArrayBuffer(str.length)
    const view = new Uint8Array(buffer)
    for (let i = 0, strLen = str.length; i < strLen; i++) {
      view[i] = str.charCodeAt(i)
    }
    return buffer
}

/**
 * Converts a UTF-8 byte buffer to a string.
 */
 export function buffer2String(buffer: ArrayBuffer): string {
    return String.fromCharCode.apply(null, new Uint8Array(buffer))
}

/**
 * Converts an ArrayBuffer to an array of bytes.
 */
 export function buffer2Array(buffer: ArrayBuffer): number[] {
    return Array.from(new Uint8Array(buffer))
}

/**
 * Converts an array of bytes to an ArrayBuffer.
 */
 export function array2Buffer(array: number[]): ArrayBuffer {
    return new Uint8Array(array).buffer
}

/**
 * Imports a RSA public key in the format exported by Hammeregg Desktop.
 */
export async function importRSAPublicKey(pem: string): Promise<CryptoKey> {
    let onelinePem = pem.replace(/[\r\n]/g, "")
    let contents = onelinePem.substring(PUBLIC_KEY_HEADER.length, onelinePem.length - PUBLIC_KEY_FOOTER.length)
    let der = string2Buffer(atob(contents))
    return crypto.subtle.importKey("spki", der, HAMMEREGG_RSA_PARAMS, false, ["wrapKey"])
}

/**
 * Imports a RSA private key in the format exported by Hammeregg Desktop.
 */
 export async function importRSAPrivateKey(pem: string): Promise<CryptoKey> {
    let onelinePem = pem.replace(/[\r\n]/g, "")
    let contents = onelinePem.substring(PRIVATE_KEY_HEADER.length, onelinePem.length - PRIVATE_KEY_FOOTER.length)
    let der = string2Buffer(atob(contents))
    return crypto.subtle.importKey("pkcs8", der, HAMMEREGG_RSA_PARAMS, false, ["unwrapKey"])
}

/**
 * Generates an AES secret key in the format expected by Hammeregg Desktop.
 */
export async function generateAESKey(): Promise<CryptoKey> {
    return crypto.subtle.generateKey(HAMMEREGG_AES_PARAMS, true, ["encrypt"])
}

/**
 * Generates a 96-bit AES init vector.
 */
export function generateIV(): Uint8Array {
    return crypto.getRandomValues(new Uint8Array(12))
}