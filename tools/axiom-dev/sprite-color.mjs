import { shiftHue } from "./sprite-sheet.mjs";

const clamp = value => Math.max(0, Math.min(1, value));
const rgb = pixel => [pixel >>> 24, (pixel >>> 16) & 255, (pixel >>> 8) & 255].map(channel => channel / 255);

export function adjustColor(pixel, { hue, saturation, contrast, brightness = 0, gamma = 1, filter }) {
  const alpha = pixel & 255;
  if (!alpha || (!hue && filter === "None" && saturation === 1 && contrast === 1 && !brightness && gamma === 1)) return pixel;
  let channels = rgb(shiftHue(pixel, hue));
  const [r, g, b] = channels;
  const gray = .2126 * r + .7152 * g + .0722 * b;
  switch (filter) {
    case "Grayscale": channels = [gray, gray, gray]; break;
    case "Sepia": channels = [gray * 1.2, gray, gray * .76]; break;
    case "Invert": channels = channels.map(channel => 1 - channel); break;
    case "Posterize": channels = channels.map(channel => Math.round(channel * 4) / 4); break;
  }
  const luma = .2126 * channels[0] + .7152 * channels[1] + .0722 * channels[2];
  const [red, green, blue] = channels.map(channel => {
    const saturated = luma + (channel - luma) * saturation;
    return Math.round(255 * clamp((saturated - .5) * contrast + .5 + brightness) ** (1 / gamma));
  });
  return ((red << 24) | (green << 16) | (blue << 8) | alpha) >>> 0;
}

// Direct linear sRGB matrices from Björn Ottosson's public-domain Oklab reference.
// https://bottosson.github.io/posts/oklab/
function toLab(channels) {
  const [r, g, b] = channels.map(c => c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4);
  const l = Math.cbrt(.4122214708 * r + .5363325363 * g + .0514459929 * b);
  const m = Math.cbrt(.2119034982 * r + .6806995451 * g + .1073969566 * b);
  const s = Math.cbrt(.0883024619 * r + .2817188376 * g + .6299787005 * b);
  return [.2104542553 * l + .7936177850 * m - .0040720468 * s,
    1.9779984951 * l - 2.4285922050 * m + .4505937099 * s,
    .0259040371 * l + .7827717662 * m - .8086757660 * s];
}

function fromLab(lightness, chroma, hue) {
  const a = chroma * Math.cos(hue), b = chroma * Math.sin(hue);
  const l = (lightness + .3963377774 * a + .2158037573 * b) ** 3;
  const m = (lightness - .1055613458 * a - .0638541728 * b) ** 3;
  const s = (lightness - .0894841775 * a - 1.2914855480 * b) ** 3;
  return [4.0767416621 * l - 3.3077115913 * m + .2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - .3413193965 * s,
    -.0041960863 * l - .7034186147 * m + 1.7076147010 * s];
}

export function paletteCandidates(seed, range, center = 0) {
  return Array.from({ length: 6 }, (_, i) => {
    let hash = Math.imul((seed | 0) + i + 1, 0x9e3779b1);
    hash = Math.imul(hash ^ (hash >>> 16), 0x85ebca6b);
    const unit = ((hash ^ (hash >>> 13)) >>> 0) / 4294967296;
    return Math.round(center + (2 * unit - 1) * range);
  });
}

export function varyColor(pixel, { offset, mode, strength, shadow, highlight, chromaLimit, protectNeutrals }) {
  if (!(pixel & 255) || !strength) return pixel;
  const [lightness, a, b] = toLab(rgb(pixel));
  const chroma = Math.hypot(a, b);
  if (protectNeutrals && chroma < .028) return pixel;
  const dark = Math.max(0, 1 - 2 * lightness), bright = Math.max(0, 2 * lightness - 1);
  const accent = clamp((chroma - .055) / .10);
  const modes = { coherent: [1, 0], ramp: [.55, 1], accents: [accent, 0], material: [.4, .45], mood: [.7, .65], wild: [1.3, 1.2] };
  const [rotation, bias] = modes[mode];
  const angle = Math.atan2(b, a) + (offset * rotation + (shadow * dark + highlight * bright) * bias) * strength * Math.PI / 180;
  let high = Math.min(chromaLimit, chroma * (mode === "mood" ? 1 - .3 * strength : 1)), low = 0;
  let converted = fromLab(lightness, high, angle);
  if (converted.some(c => c < 0 || c > 1)) {
    // Find an in-gamut chroma without changing the lightness of the ramp.
    for (let step = 0; step < 13; step++) {
      const middle = (low + high) / 2;
      const sample = fromLab(lightness, middle, angle);
      if (sample.some(c => c < 0 || c > 1)) high = middle;
      else low = middle;
    }
    converted = fromLab(lightness, low, angle);
  }
  const [r, g, blue] = converted.map(c => Math.round(255 * clamp(c <= .0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - .055)));
  return ((r << 24) | (g << 16) | (blue << 8) | (pixel & 255)) >>> 0;
}
