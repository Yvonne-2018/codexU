export interface SurfaceTintTokens {
  color: string;
  maximumOpacity: number;
}

export interface WindowsGlassVisualTokens {
  windowBg: string;
  pageBg: string;
  toolbarBg: string;
  cardBg: string;
  cardBgStrong: string;
  controlBg: string;
  focusSurface: string;
  statusSurface: string;
  glassFallbackBg: string;
  paletteSurfaceTintActive: string;
  surface: string;
  surfaceElevated: string;
  surfaceElevatedStrong: string;
  surfaceInset: string;
  textPrimary: string;
  textSecondary: string;
  textTertiary: string;
  border: string;
  statusOk: string;
  statusWarn: string;
  statusError: string;
  shadowSoft: string;
}

const FIXED_VISUAL_TOKENS = {
  light: {
    surface: '#ffffff',
    surfaceElevated: 'rgba(255, 255, 255, 0.78)',
    surfaceElevatedStrong: 'rgba(255, 255, 255, 0.92)',
    surfaceInset: 'rgba(255, 255, 255, 0.62)',
    windowBg: 'rgba(248, 250, 252, 0.72)',
    pageBg: (surfaceTint: string) =>
      `radial-gradient(circle at top left, ${surfaceTint}, transparent 36rem), linear-gradient(135deg, rgba(255, 255, 255, 0.78), rgba(241, 245, 249, 0.58))`,
    toolbarBg: 'rgba(255, 255, 255, 0.66)',
    cardBg: 'rgba(255, 255, 255, 0.72)',
    cardBgStrong: 'rgba(255, 255, 255, 0.9)',
    controlBg: 'rgba(255, 255, 255, 0.58)',
    focusSurface: 'rgba(40, 102, 247, 0.7)',
    statusSurface: 'rgba(255, 255, 255, 0.56)',
    glassFallbackBg: 'rgba(255, 255, 255, 0.96)',
    textPrimary: '#111827',
    textSecondary: '#4b5563',
    textTertiary: '#6b7280',
    border: 'rgba(148, 163, 184, 0.30)',
    statusOk: '#16a34a',
    statusWarn: '#d97706',
    statusError: '#dc2626',
    shadowSoft: '0 16px 42px -30px rgba(15, 23, 42, 0.33)',
  },
  dark: {
    surface: '#0f172a',
    surfaceElevated: 'rgba(15, 23, 42, 0.78)',
    surfaceElevatedStrong: 'rgba(15, 23, 42, 0.92)',
    surfaceInset: 'rgba(15, 23, 42, 0.62)',
    windowBg: 'rgba(2, 6, 23, 0.82)',
    pageBg: (surfaceTint: string) =>
      `radial-gradient(circle at top left, ${surfaceTint}, transparent 34rem), linear-gradient(135deg, rgba(15, 23, 42, 0.76), rgba(2, 6, 23, 0.7))`,
    toolbarBg: 'rgba(15, 23, 42, 0.68)',
    cardBg: 'rgba(15, 23, 42, 0.72)',
    cardBgStrong: 'rgba(15, 23, 42, 0.9)',
    controlBg: 'rgba(30, 41, 59, 0.62)',
    focusSurface: 'rgba(123, 160, 255, 0.78)',
    statusSurface: 'rgba(15, 23, 42, 0.58)',
    glassFallbackBg: 'rgba(15, 23, 42, 0.96)',
    textPrimary: '#f8fafc',
    textSecondary: '#cbd5e1',
    textTertiary: '#94a3b8',
    border: 'rgba(148, 163, 184, 0.32)',
    statusOk: '#30d158',
    statusWarn: '#f59e0b',
    statusError: '#ff453a',
    shadowSoft: '0 16px 42px -30px rgba(2, 6, 23, 0.55)',
  },
} as const;

const clampHexAlpha = (hex: string, alpha: number): string => {
  const normalized = hex.replace('#', '').trim();
  if (!/^[0-9a-fA-F]{6}$/.test(normalized)) {
    return hex;
  }

  const clamped = Math.max(0, Math.min(1, alpha));
  return `#${normalized}${Math.round(clamped * 255)
    .toString(16)
    .padStart(2, '0')
    .toUpperCase()}`;
};

export function resolveWindowsGlassVisualTokens(
  isDark: boolean,
  surfaceTint: SurfaceTintTokens,
): WindowsGlassVisualTokens {
  const base = isDark ? FIXED_VISUAL_TOKENS.dark : FIXED_VISUAL_TOKENS.light;
  const paletteSurfaceTintActive = clampHexAlpha(
    surfaceTint.color,
    surfaceTint.maximumOpacity,
  );

  return {
    ...base,
    pageBg: base.pageBg(paletteSurfaceTintActive),
    paletteSurfaceTintActive,
  };
}
