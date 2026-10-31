/** Stable SDK error categories shared by native adapters, Worker and planning. */
export interface ImageDecodeDetails {encoding:string;width:number;height:number;targetWidth:number|null;targetHeight:number|null;targetDecodeSupported:boolean;unsupportedReason:string|null;jpegProcess:string|null;maskKind:string|null;maskWidth:number|null;maskHeight:number|null;maskEncoding:string|null;maskBitsPerComponent:number|null;mattePresent:boolean;}
export interface ResourceDetails {reason:string;stage:string;limit?:number;actual?:number;image?:ImageDecodeDetails;}
export type ErrorCode = "format" | "password_required" | "password_invalid" | "permission" | "unsupported" | "font" | "resource_limit" | "invalid_handle" | "invalid_argument" | "internal" | "cancelled";
export class PdfApiError extends Error {
  override name = "PdfApiError";
  constructor(readonly code: ErrorCode, message: string, readonly operation: string, readonly page?: number, readonly taskId?: number, readonly session?: string, readonly resource?: ResourceDetails) { super(message); }
}
