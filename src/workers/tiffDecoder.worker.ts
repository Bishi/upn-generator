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

function tiffToRgba(ifd: UTIF.IFD) {
  const photometricInterpretation = firstTiffTagNumber(ifd.t262 ?? [2]);
  if (photometricInterpretation !== 5) return UTIF.toRGBA8(ifd);

  const bitsPerSample = firstTiffTagNumber(ifd.t258 ?? [8]);
  const samplesPerPixel = firstTiffTagNumber(ifd.t277 ?? [4]);
  if (
    bitsPerSample !== 8
    || !Number.isSafeInteger(samplesPerPixel)
    || samplesPerPixel < 4
    || samplesPerPixel > 8
  ) {
    throw new Error("This CMYK TIFF sample layout is not supported.");
  }

  const pixelCount = ifd.width * ifd.height;
  if (ifd.data.length < pixelCount * samplesPerPixel) {
    throw new Error("The CMYK TIFF pixel data is incomplete.");
  }
  const rgba = new Uint8Array(pixelCount * 4);
  for (let pixel = 0; pixel < pixelCount; pixel += 1) {
    const source = pixel * samplesPerPixel;
    const target = pixel * 4;
    const cyan = 255 - ifd.data[source];
    const magenta = 255 - ifd.data[source + 1];
    const yellow = 255 - ifd.data[source + 2];
    const black = (255 - ifd.data[source + 3]) / 255;
    rgba[target] = Math.round(cyan * black);
    rgba[target + 1] = Math.round(magenta * black);
    rgba[target + 2] = Math.round(yellow * black);
    rgba[target + 3] = samplesPerPixel > 4 ? ifd.data[source + 4] : 255;
  }
  return rgba;
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

    const rgba = tiffToRgba(ifd);
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
