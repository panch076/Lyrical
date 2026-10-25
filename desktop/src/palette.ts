export type RGB = [number, number, number];

export const FALLBACK_PALETTE: RGB[] = [
  [250, 61, 92], [158, 41, 122], [74, 26, 107],
  [214, 51, 102], [117, 33, 117], [51, 26, 92],
  [153, 36, 107], [77, 26, 102], [31, 15, 61],
];

export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = reject;
    image.src = src;
  });
}

/** Nine colours from a 3×3 downsample of the artwork. */
export function extractPalette(image: HTMLImageElement): RGB[] {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 3;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return FALLBACK_PALETTE;
  context.imageSmoothingQuality = "medium";
  context.drawImage(image, 0, 0, 3, 3);
  const data = context.getImageData(0, 0, 3, 3).data;
  const colors: RGB[] = [];
  for (let i = 0; i < data.length; i += 4) colors.push([data[i], data[i + 1], data[i + 2]]);
  return colors;
}

export const rgb = ([r, g, b]: RGB, alpha = 1) => `rgba(${r}, ${g}, ${b}, ${alpha})`;

/** Mixes toward the accent but keeps it dark enough for white text. */
export function accent(palette: RGB[]): RGB {
  const [r, g, b] = palette[4] ?? FALLBACK_PALETTE[4];
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  const k = luminance > 0.55 ? 0.55 / luminance : 1;
  return [Math.round(r * k), Math.round(g * k), Math.round(b * k)];
}
