import * as UTIF from "utif2";

const MAX_TIFF_PIXELS = 40_000_000;

type TiffWorkerResponse =
  | { ok: true; width: number; height: number; rgba: ArrayBuffer }
  | { ok: false; error: string };

interface TiffWorkerScope {
  onmessage: ((event: MessageEvent<ArrayBuffer>) => void) | null;
  postMessage(message: TiffWorkerResponse, transfer?: Transferable[]): void;
}

const workerScope = self as unknown as TiffWorkerScope;

function firstTiffTagNumber(value: unknown) {
  if (Array.isArray(value) || value instanceof Uint8Array) {
    return Number(value[0] ?? 0);
  }
  return Number(value ?? 0);
}

function validDimensions(width: number, height: number) {
  return Number.isSafeInteger(width)
    && Number.isSafeInteger(height)
    && width > 0
    && height > 0
    && width * height <= MAX_TIFF_PIXELS;
}

workerScope.onmessage = ({ data: bytes }) => {
  try {
    const ifd = UTIF.decode(bytes)[0];
    if (!ifd) throw new Error("The TIFF does not contain a displayable image.");

    const taggedWidth = firstTiffTagNumber(ifd.t256);
    const taggedHeight = firstTiffTagNumber(ifd.t257);
    if (!validDimensions(taggedWidth, taggedHeight)) {
      throw new Error("The TIFF dimensions are invalid or too large to display safely.");
    }

    UTIF.decodeImage(bytes, ifd);
    if (!validDimensions(ifd.width, ifd.height)) {
      throw new Error("The decoded TIFF dimensions are invalid or too large to display safely.");
    }

    const rgba = UTIF.toRGBA8(ifd);
    const rgbaBuffer = rgba.slice().buffer as ArrayBuffer;
    workerScope.postMessage(
      { ok: true, width: ifd.width, height: ifd.height, rgba: rgbaBuffer },
      [rgbaBuffer],
    );
  } catch (reason) {
    const error = reason instanceof Error ? reason.message : String(reason);
    workerScope.postMessage({ ok: false, error });
  }
};
