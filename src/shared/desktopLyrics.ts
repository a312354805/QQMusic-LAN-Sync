export type DesktopLyricsFontWeight = 400 | 500 | 600 | 700 | 800
export type DesktopLyricsAlignment = 'left' | 'center' | 'right'
export type DesktopLyricsTrayAction = 'toggle_enabled' | 'toggle_locked'

export type DesktopLyricsSettings = {
  enabled: boolean
  alwaysOnTop: boolean
  locked: boolean
  fontFamily: string
  fontSize: number
  fontWeight: DesktopLyricsFontWeight
  alignment: DesktopLyricsAlignment
  activeColor: string
  inactiveColor: string
  translationColor: string
  romanizationColor: string
  backgroundColor: string
  backgroundOpacity: number
  backgroundBlur: number
  borderRadius: number
  showNextLine: boolean
  showTranslation: boolean
  showRomanization: boolean
}

export type DesktopLyricsColorPreset = {
  id: string
  name: string
  colors: Pick<DesktopLyricsSettings,
    'activeColor' | 'inactiveColor' | 'translationColor' | 'romanizationColor' | 'backgroundColor'>
}

export const desktopLyricsStorageKey = 'qqmusic-lan-sync:desktop-lyrics'
const desktopLyricsStorageVersionKey = `${desktopLyricsStorageKey}:version`
const desktopLyricsStorageVersion = '2'
export const desktopLyricsSettingsEvent = 'desktop-lyrics://settings'
export const desktopLyricsTrayActionEvent = 'desktop-lyrics://tray-action'

export const defaultDesktopLyricsSettings: DesktopLyricsSettings = {
  enabled: false,
  alwaysOnTop: true,
  locked: false,
  fontFamily: '"Segoe UI Variable", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif',
  fontSize: 40,
  fontWeight: 800,
  alignment: 'center',
  activeColor: '#b9f35b',
  inactiveColor: '#edf0e7',
  translationColor: '#73d9d0',
  romanizationColor: '#e1aa72',
  backgroundColor: '#151914',
  backgroundOpacity: 0,
  backgroundBlur: 14,
  borderRadius: 12,
  showNextLine: true,
  showTranslation: true,
  showRomanization: false,
}

export const desktopLyricsColorPresets: DesktopLyricsColorPreset[] = [
  {
    id: 'lime',
    name: '青柠绿',
    colors: {
      activeColor: '#b9f35b',
      inactiveColor: '#edf0e7',
      translationColor: '#73d9d0',
      romanizationColor: '#e1aa72',
      backgroundColor: '#151914',
    },
  },
  {
    id: 'sky',
    name: '天空蓝',
    colors: {
      activeColor: '#78c9ff',
      inactiveColor: '#e5f3ff',
      translationColor: '#a8d8ff',
      romanizationColor: '#c7c4ff',
      backgroundColor: '#111a22',
    },
  },
  {
    id: 'aurora',
    name: '极光青',
    colors: {
      activeColor: '#65e4d2',
      inactiveColor: '#ddfffa',
      translationColor: '#91d7ff',
      romanizationColor: '#ffd38c',
      backgroundColor: '#10201d',
    },
  },
  {
    id: 'coral',
    name: '珊瑚红',
    colors: {
      activeColor: '#ff8b7b',
      inactiveColor: '#fff0ec',
      translationColor: '#ffd19a',
      romanizationColor: '#9de0d8',
      backgroundColor: '#231513',
    },
  },
  {
    id: 'classic-gold',
    name: '经典金',
    colors: {
      activeColor: '#ffe45e',
      inactiveColor: '#fff7cf',
      translationColor: '#80d8ff',
      romanizationColor: '#ffb3d1',
      backgroundColor: '#1e1b10',
    },
  },
  {
    id: 'moonlight',
    name: '月光白',
    colors: {
      activeColor: '#ffffff',
      inactiveColor: '#cbd5e1',
      translationColor: '#93c5fd',
      romanizationColor: '#f9a8d4',
      backgroundColor: '#111418',
    },
  },
  {
    id: 'sakura',
    name: '樱花粉',
    colors: {
      activeColor: '#ff8fbd',
      inactiveColor: '#ffe3ef',
      translationColor: '#c4b5fd',
      romanizationColor: '#7dd3c7',
      backgroundColor: '#21131b',
    },
  },
  {
    id: 'sunset',
    name: '暖阳橙',
    colors: {
      activeColor: '#ffb04a',
      inactiveColor: '#fff0d2',
      translationColor: '#ff8c7a',
      romanizationColor: '#8fe1c2',
      backgroundColor: '#25170f',
    },
  },
]

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.min(maximum, Math.max(minimum, value))

const numberValue = (value: unknown, fallback: number, minimum: number, maximum: number) => {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? clamp(numeric, minimum, maximum) : fallback
}

const booleanValue = (value: unknown, fallback: boolean) =>
  typeof value === 'boolean' ? value : fallback

const colorValue = (value: unknown, fallback: string) =>
  typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value.trim()) ? value.trim() : fallback

const fontWeights: DesktopLyricsFontWeight[] = [400, 500, 600, 700, 800]

export const normalizeDesktopLyricsSettings = (
  value: Partial<DesktopLyricsSettings> | null | undefined,
): DesktopLyricsSettings => {
  const defaults = defaultDesktopLyricsSettings
  const fontWeight = Number(value?.fontWeight)
  const alignment = value?.alignment

  return {
    enabled: booleanValue(value?.enabled, defaults.enabled),
    alwaysOnTop: booleanValue(value?.alwaysOnTop, defaults.alwaysOnTop),
    locked: booleanValue(value?.locked, defaults.locked),
    fontFamily: typeof value?.fontFamily === 'string' && value.fontFamily.trim()
      ? value.fontFamily.trim().slice(0, 240)
      : defaults.fontFamily,
    fontSize: numberValue(value?.fontSize, defaults.fontSize, 24, 72),
    fontWeight: fontWeights.includes(fontWeight as DesktopLyricsFontWeight)
      ? fontWeight as DesktopLyricsFontWeight
      : defaults.fontWeight,
    alignment: alignment === 'left' || alignment === 'right' ? alignment : 'center',
    activeColor: colorValue(value?.activeColor, defaults.activeColor),
    inactiveColor: colorValue(value?.inactiveColor, defaults.inactiveColor),
    translationColor: colorValue(value?.translationColor, defaults.translationColor),
    romanizationColor: colorValue(value?.romanizationColor, defaults.romanizationColor),
    backgroundColor: colorValue(value?.backgroundColor, defaults.backgroundColor),
    backgroundOpacity: numberValue(value?.backgroundOpacity, defaults.backgroundOpacity, 0, 0.95),
    backgroundBlur: numberValue(value?.backgroundBlur, defaults.backgroundBlur, 0, 40),
    borderRadius: numberValue(value?.borderRadius, defaults.borderRadius, 0, 28),
    showNextLine: booleanValue(value?.showNextLine, defaults.showNextLine),
    showTranslation: booleanValue(value?.showTranslation, defaults.showTranslation),
    showRomanization: booleanValue(value?.showRomanization, defaults.showRomanization),
  }
}

export const loadDesktopLyricsSettings = (): DesktopLyricsSettings => {
  try {
    const saved = window.localStorage.getItem(desktopLyricsStorageKey)
    if (!saved) return defaultDesktopLyricsSettings
    const parsed = JSON.parse(saved) as Partial<DesktopLyricsSettings>
    const storedVersion = window.localStorage.getItem(desktopLyricsStorageVersionKey)
    const normalized = normalizeDesktopLyricsSettings(storedVersion === desktopLyricsStorageVersion
      ? parsed
      : { ...parsed, backgroundOpacity: 0 })
    if (storedVersion !== desktopLyricsStorageVersion) {
      window.localStorage.setItem(desktopLyricsStorageKey, JSON.stringify(normalized))
      window.localStorage.setItem(desktopLyricsStorageVersionKey, desktopLyricsStorageVersion)
    }
    return normalized
  } catch {
    return defaultDesktopLyricsSettings
  }
}

export const saveDesktopLyricsSettings = (settings: DesktopLyricsSettings) => {
  const normalized = normalizeDesktopLyricsSettings(settings)
  try {
    window.localStorage.setItem(desktopLyricsStorageKey, JSON.stringify(normalized))
    window.localStorage.setItem(desktopLyricsStorageVersionKey, desktopLyricsStorageVersion)
  } catch {
    // The current session still uses the updated values if persistence is unavailable.
  }
  window.dispatchEvent(new CustomEvent(desktopLyricsSettingsEvent, { detail: normalized }))
  return normalized
}
