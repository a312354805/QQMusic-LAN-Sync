import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Monitor,
  Palette,
  RotateCcw,
  SlidersHorizontal,
  Type,
} from 'lucide-react'
import clsx from 'clsx'
import { useEffect, useMemo, useState, type CSSProperties } from 'react'
import { api } from '../shared/api'
import {
  desktopLyricsColorPresets,
  type DesktopLyricsAlignment,
  type DesktopLyricsFontWeight,
  type DesktopLyricsSettings,
} from '../shared/desktopLyrics'
import type { LyricsDocument, LyricsLine, LyricsTrack } from '../shared/types'
import './DesktopLyricsSettingsView.scss'

type Props = {
  settings: DesktopLyricsSettings
  document: LyricsDocument | null
  positionMs: number
  title: string | null
  artist: string | null
  error: string | null
  update: (patch: Partial<DesktopLyricsSettings>) => void
  reset: () => void
}

const findAlignedLine = (track: LyricsTrack | null, line: LyricsLine | null) => {
  if (!track || !line) return null
  const exact = track.lines.find((candidate) => candidate.startMs === line.startMs && candidate.text.trim())
  if (exact) return exact

  return track.lines
    .map((candidate) => ({ candidate, delta: Math.abs(candidate.startMs - line.startMs) }))
    .filter(({ candidate, delta }) => candidate.text.trim() && delta <= 500)
    .sort((left, right) => left.delta - right.delta)[0]?.candidate ?? null
}

const colorWithOpacity = (color: string, opacity: number) => {
  const match = color.match(/^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i)
  if (!match) return 'transparent'
  return `rgba(${Number.parseInt(match[1], 16)}, ${Number.parseInt(match[2], 16)}, ${Number.parseInt(match[3], 16)}, ${opacity})`
}

function ToggleSetting({ label, description, checked, disabled = false, onChange }: {
  label: string
  description?: string
  checked: boolean
  disabled?: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <label className="desktop-setting-row desktop-setting-toggle" data-disabled={disabled || undefined}>
      <span>
        <strong>{label}</strong>
        {description && <small>{description}</small>}
      </span>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <i aria-hidden="true" />
    </label>
  )
}

function RangeSetting({ label, value, minimum, maximum, step = 1, suffix, onChange }: {
  label: string
  value: number
  minimum: number
  maximum: number
  step?: number
  suffix: string
  onChange: (value: number) => void
}) {
  return (
    <label className="desktop-setting-row desktop-setting-range">
      <strong>{label}</strong>
      <span>
        <input
          type="range"
          min={minimum}
          max={maximum}
          step={step}
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
        />
        <output>{step < 1 ? Math.round(value * 100) : value}{suffix}</output>
      </span>
    </label>
  )
}

function ColorSetting({ label, value, onChange }: {
  label: string
  value: string
  onChange: (value: string) => void
}) {
  return (
    <label className="desktop-setting-row desktop-setting-color">
      <strong>{label}</strong>
      <span>
        <input type="color" aria-label={label} value={value} onChange={(event) => onChange(event.target.value)} />
        <code>{value}</code>
      </span>
    </label>
  )
}

export function DesktopLyricsSettingsView({
  settings,
  document,
  positionMs,
  title,
  artist,
  error,
  update,
  reset,
}: Props) {
  const [fontDraft, setFontDraft] = useState(settings.fontFamily)
  const [actionError, setActionError] = useState<string | null>(null)
  const lines = useMemo(() => document?.original.lines ?? [], [document])
  const adjustedPosition = positionMs + (document?.offsetMs ?? 0)

  useEffect(() => setFontDraft(settings.fontFamily), [settings.fontFamily])

  const activeIndex = useMemo(() => {
    let found = -1
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].startMs > adjustedPosition) break
      found = index
    }
    return found
  }, [adjustedPosition, lines])

  const currentLine = lines[activeIndex] ?? null
  const nextLine = lines[activeIndex + 1] ?? null
  const translation = findAlignedLine(document?.translation ?? null, currentLine)
  const romanization = findAlignedLine(document?.romanization ?? null, currentLine)
  const activePreset = desktopLyricsColorPresets.find((preset) =>
    Object.entries(preset.colors).every(([key, value]) => settings[key as keyof typeof preset.colors] === value),
  )

  const commitFont = () => {
    const value = fontDraft.trim()
    if (value) update({ fontFamily: value })
    else setFontDraft(settings.fontFamily)
  }

  const resetPosition = async () => {
    setActionError(null)
    try {
      await api.resetDesktopLyricsPosition()
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  return (
    <div className="desktop-settings-view">
      <header className="desktop-settings-heading">
        <div>
          <span className="eyebrow">歌词样式</span>
          <h2>桌面歌词</h2>
          <p>配置悬浮窗的字体、配色和显示方式，修改后会立即应用。</p>
        </div>
        <button type="button" className="settings-command" onClick={reset}>
          <RotateCcw size={15} />恢复默认
        </button>
      </header>

      {(error || actionError) && <div className="settings-inline-error" role="alert">{error ?? actionError}</div>}

      <section className="desktop-lyrics-preview" aria-label="桌面歌词预览" style={{
        '--preview-font-family': settings.fontFamily,
        '--preview-font-size': `${settings.fontSize}px`,
        '--preview-font-weight': settings.fontWeight,
        '--preview-text-align': settings.alignment,
        '--preview-active': settings.activeColor,
        '--preview-inactive': settings.inactiveColor,
        '--preview-translation': settings.translationColor,
        '--preview-romanization': settings.romanizationColor,
        '--preview-background': colorWithOpacity(settings.backgroundColor, settings.backgroundOpacity),
        '--preview-blur': `${settings.backgroundBlur}px`,
        '--preview-radius': `${settings.borderRadius}px`,
      } as CSSProperties}>
        <div className="desktop-lyrics-preview__meta">
          <span><Monitor size={15} />实时预览</span>
          <em>{settings.enabled ? '桌面歌词已启用' : '桌面歌词未启用'}</em>
        </div>
        <div className="desktop-lyrics-preview__surface">
          <strong>{currentLine?.text ?? title ?? '故事的小黄花'}</strong>
          {settings.showTranslation && <small data-kind="translation">{translation?.text ?? 'The current lyric appears here'}</small>}
          {settings.showRomanization && <small data-kind="romanization">{romanization?.text ?? 'ge ci luo ma yin'}</small>}
          {settings.showNextLine && <span>{nextLine?.text ?? artist ?? '下一句歌词'}</span>}
        </div>
      </section>

      <section className="desktop-settings-section" aria-labelledby="desktop-window-title">
        <header><Monitor size={18} /><div><h3 id="desktop-window-title">显示与窗口</h3><p>控制桌面歌词窗口和鼠标交互。</p></div></header>
        <ToggleSetting label="启用桌面歌词" description="在桌面上显示与播放器同步的歌词悬浮窗" checked={settings.enabled} onChange={(enabled) => update({ enabled })} />
        <ToggleSetting label="保持窗口置顶" description="让歌词显示在普通应用窗口上方" checked={settings.alwaysOnTop} onChange={(alwaysOnTop) => update({ alwaysOnTop })} />
        <ToggleSetting label="锁定并穿透鼠标" description="锁定后点击会穿透歌词窗口；解锁后可以拖动和调整大小" checked={settings.locked} onChange={(locked) => update({ locked })} />
        <div className="desktop-setting-actions">
          <button type="button" className="settings-command" disabled={!settings.enabled} onClick={() => void resetPosition()}>
            <RotateCcw size={14} />重置窗口位置
          </button>
        </div>
      </section>

      <section className="desktop-settings-section" aria-labelledby="desktop-palette-title">
        <header><Palette size={18} /><div><h3 id="desktop-palette-title">快捷配色</h3><p>为桌面歌词选择一套可读性较高的颜色组合。</p></div></header>
        <div className="desktop-color-presets">
          {desktopLyricsColorPresets.map((preset) => {
            const active = preset.id === activePreset?.id
            return (
              <button
                type="button"
                className={clsx(active && 'is-active')}
                aria-pressed={active}
                key={preset.id}
                onClick={() => update(preset.colors)}
              >
                <span aria-hidden="true">
                  {Object.values(preset.colors).slice(0, 4).map((color, index) => <i style={{ background: color }} key={`${color}-${index}`} />)}
                </span>
                <strong>{preset.name}</strong>
              </button>
            )
          })}
        </div>
      </section>

      <section className="desktop-settings-section" aria-labelledby="desktop-type-title">
        <header><Type size={18} /><div><h3 id="desktop-type-title">字体与排版</h3><p>使用 Windows 已安装字体，多个字体可按 CSS font-family 语法填写。</p></div></header>
        <label className="desktop-setting-text">
          <strong>字体系列</strong>
          <input
            type="text"
            value={fontDraft}
            spellCheck={false}
            onBlur={commitFont}
            onChange={(event) => setFontDraft(event.target.value)}
            onKeyDown={(event) => { if (event.key === 'Enter') event.currentTarget.blur() }}
          />
        </label>
        <RangeSetting label="主歌词字号" value={settings.fontSize} minimum={24} maximum={72} suffix=" px" onChange={(fontSize) => update({ fontSize })} />
        <label className="desktop-setting-row desktop-setting-select">
          <strong>字体粗细</strong>
          <select value={settings.fontWeight} onChange={(event) => update({ fontWeight: Number(event.target.value) as DesktopLyricsFontWeight })}>
            <option value="400">标准</option>
            <option value="500">中等</option>
            <option value="600">半粗</option>
            <option value="700">粗体</option>
            <option value="800">特粗</option>
          </select>
        </label>
        <div className="desktop-setting-row desktop-setting-alignment">
          <strong>对齐方式</strong>
          <div role="group" aria-label="桌面歌词对齐方式">
            {([
              ['left', '左对齐', AlignLeft],
              ['center', '居中对齐', AlignCenter],
              ['right', '右对齐', AlignRight],
            ] as const).map(([value, label, Icon]) => (
              <button
                type="button"
                aria-label={label}
                aria-pressed={settings.alignment === value}
                className={clsx(settings.alignment === value && 'is-active')}
                key={value}
                onClick={() => update({ alignment: value as DesktopLyricsAlignment })}
              ><Icon size={16} /></button>
            ))}
          </div>
        </div>
      </section>

      <section className="desktop-settings-section" aria-labelledby="desktop-detail-title">
        <header><SlidersHorizontal size={18} /><div><h3 id="desktop-detail-title">内容与背景</h3><p>调整辅助歌词、背景透明度和颜色细节。</p></div></header>
        <ToggleSetting label="显示下一句" checked={settings.showNextLine} onChange={(showNextLine) => update({ showNextLine })} />
        <ToggleSetting label="显示翻译" checked={settings.showTranslation} onChange={(showTranslation) => update({ showTranslation })} />
        <ToggleSetting label="显示罗马音" checked={settings.showRomanization} onChange={(showRomanization) => update({ showRomanization })} />
        <RangeSetting label="背景透明度" value={settings.backgroundOpacity} minimum={0} maximum={0.95} step={0.05} suffix="%" onChange={(backgroundOpacity) => update({ backgroundOpacity })} />
        <RangeSetting label="背景模糊" value={settings.backgroundBlur} minimum={0} maximum={40} suffix=" px" onChange={(backgroundBlur) => update({ backgroundBlur })} />
        <RangeSetting label="背景圆角" value={settings.borderRadius} minimum={0} maximum={28} suffix=" px" onChange={(borderRadius) => update({ borderRadius })} />
        <div className="desktop-color-settings">
          <ColorSetting label="当前歌词" value={settings.activeColor} onChange={(activeColor) => update({ activeColor })} />
          <ColorSetting label="未唱歌词" value={settings.inactiveColor} onChange={(inactiveColor) => update({ inactiveColor })} />
          <ColorSetting label="翻译颜色" value={settings.translationColor} onChange={(translationColor) => update({ translationColor })} />
          <ColorSetting label="罗马音颜色" value={settings.romanizationColor} onChange={(romanizationColor) => update({ romanizationColor })} />
          <ColorSetting label="背景颜色" value={settings.backgroundColor} onChange={(backgroundColor) => update({ backgroundColor })} />
        </div>
      </section>
    </div>
  )
}
