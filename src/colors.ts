import type { ColorKey } from "./types";

export const COLOR_KEYS: ColorKey[] = [
  "default",
  "red",
  "orange",
  "yellow",
  "green",
  "teal",
  "blue",
  "darkblue",
  "purple",
  "pink",
  "brown",
  "gray",
];

export const COLOR_LABELS: Record<ColorKey, string> = {
  default: "Default",
  red: "Coral",
  orange: "Peach",
  yellow: "Sand",
  green: "Mint",
  teal: "Sage",
  blue: "Fog",
  darkblue: "Storm",
  purple: "Dusk",
  pink: "Blossom",
  brown: "Clay",
  gray: "Chalk",
};

export const LIGHT_HEX: Record<ColorKey, string> = {
  default: "#FFFFFF",
  red: "#FAAFA8",
  orange: "#F39F76",
  yellow: "#FFF8B8",
  green: "#E2F6D3",
  teal: "#B4DDD3",
  blue: "#D4E4ED",
  darkblue: "#AECCDC",
  purple: "#D3BFDB",
  pink: "#F6E2DD",
  brown: "#E9E3D4",
  gray: "#EFEFF1",
};

export const DARK_HEX: Record<ColorKey, string> = {
  default: "#202124",
  red: "#5C2B29",
  orange: "#614A19",
  yellow: "#635D19",
  green: "#345920",
  teal: "#16504B",
  blue: "#2D555E",
  darkblue: "#1E3A5F",
  purple: "#42275E",
  pink: "#5B2245",
  brown: "#442F19",
  gray: "#3C3F43",
};

export function bgFor(color: ColorKey, dark: boolean): string {
  return dark ? DARK_HEX[color] : LIGHT_HEX[color];
}

export function borderFor(color: ColorKey, dark: boolean): string {
  if (color !== "default") return "transparent";
  return dark ? "#5f6368" : "#e0e0e0";
}
