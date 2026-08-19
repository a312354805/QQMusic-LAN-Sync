import { getCurrentWindow } from '@tauri-apps/api/window'
import { useMemo, type CSSProperties, type PointerEvent } from 'react'
import { useDesktopLyricsSettings } from '../hooks/useDesktopLyricsSettings'
import { useRuntime } from '../hooks/useRuntime'
import type { LyricsLine, LyricsTrack } from '../shared/types'
import { isTauriRuntime } from '../shared/api'
import './DesktopLyricsOverlay.scss'

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.min(maximum, Math.max(minimum, value))

const findAlignedLine = (track: LyricsTrack | null, line: LyricsLine | null) => {
  if (!track || !line) return null
  const exact = track.lines.find((candidate) => candidate.startMs === line.startMs && candidate.text.trim())
  if (exact) return exact

  let nearest: LyricsLine | null = null
  let nearestDelta = 501
  for (const candidate of track.lines) {
    if (!candidate.text.trim()) continue
    const delta = Math.abs(candidate.startMs - line.startMs)
    if (delta < nearestDelta) {
      nearest = candidate
      nearestDelta = delta
    }
  }
  return nearestDelta <= 500 ? nearest : null
}

const colorWithOpacity = (color: string, opacity: number) => {
  const match = color.match(/^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i)
  if (!match) return 'transparent'
  const [, red, green, blue] = match
  return `rgba(${Number.parseInt(red, 16)}, ${Number.parseInt(green, 16)}, ${Number.parseInt(blue, 16)}, ${opacity})`
}

function KaraokeLine({ line, fallback, positionMs }: {
  line: LyricsLine | null
  fallback: string
  positionMs: number
}) {
  if (!line?.words?.length) return <>{line?.text || fallback}</>

  return (
    <span className="desktop-lyrics__karaoke" aria-label={line.text}>
      {line.words.map((word, index) => {
        const duration = Math.max(1, word.endMs - word.startMs)
        const progress = clamp((positionMs - word.startMs) / duration, 0, 1)
        return (
          <span
            aria-hidden="true"
            className="desktop-lyrics__word"
            data-text={word.text}
            key={`${word.startMs}-${index}`}
            style={{ '--word-progress': `${progress * 100}%` } as CSSProperties}
          >
            {word.text}
          </span>
        )
      })}
    </span>
  )
}

export function DesktopLyricsOverlay() {
  const { runtime, positionMs } = useRuntime()
  const { settings } = useDesktopLyricsSettings()
  const document = runtime?.lyrics ?? null
  const playback = runtime?.playback ?? null
  const lines = useMemo(() => document?.original.lines ?? [], [document])
  const adjustedPositionMs = positionMs + (document?.offsetMs ?? 0)

  const activeIndex = useMemo(() => {
    let found = -1
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].startMs > adjustedPositionMs) break
      found = index
    }
    return found
  }, [adjustedPositionMs, lines])

  const currentLine = lines[activeIndex] ?? null
  const nextLine = lines[activeIndex + 1] ?? null
  const translation = settings.showTranslation
    ? findAlignedLine(document?.translation ?? null, currentLine)
    : null
  const romanization = settings.showRomanization
    ? findAlignedLine(document?.romanization ?? null, currentLine)
    : null
  const fallback = playback?.title ?? '等待 QQ 音乐播放歌曲'

  const startDragging = (event: PointerEvent<HTMLElement>) => {
    if (settings.locked || event.button !== 0 || !isTauriRuntime()) return
    void getCurrentWindow().startDragging()
  }

  return (
    <main
      className="desktop-lyrics-overlay"
      data-locked={settings.locked}
      onPointerDown={startDragging}
      style={{
        '--desktop-font-family': settings.fontFamily,
        '--desktop-font-size': `${settings.fontSize}px`,
        '--desktop-font-weight': settings.fontWeight,
        '--desktop-text-align': settings.alignment,
        '--desktop-active-color': settings.activeColor,
        '--desktop-inactive-color': settings.inactiveColor,
        '--desktop-translation-color': settings.translationColor,
        '--desktop-romanization-color': settings.romanizationColor,
        '--desktop-background': colorWithOpacity(settings.backgroundColor, settings.backgroundOpacity),
        '--desktop-background-blur': `${settings.backgroundOpacity > 0 ? settings.backgroundBlur : 0}px`,
        '--desktop-radius': `${settings.borderRadius}px`,
        '--desktop-secondary-size': `${Math.max(13, Math.round(settings.fontSize * 0.38))}px`,
        '--desktop-next-size': `${Math.max(16, Math.round(settings.fontSize * 0.64))}px`,
      } as CSSProperties}
    >
      <section className="desktop-lyrics__surface" aria-label="桌面歌词">
        <div className="desktop-lyrics__content">
          <p className="desktop-lyrics__active">
            <KaraokeLine line={currentLine} fallback={fallback} positionMs={adjustedPositionMs} />
          </p>
          {(translation || romanization) && (
            <div className="desktop-lyrics__supporting">
              {translation && <small data-kind="translation">{translation.text}</small>}
              {romanization && <small data-kind="romanization">{romanization.text}</small>}
            </div>
          )}
          {settings.showNextLine && nextLine && <p className="desktop-lyrics__next">{nextLine.text}</p>}
        </div>
      </section>
      <span className="sr-only" aria-live="polite" aria-atomic="true">
        {currentLine?.text ?? fallback}
      </span>
    </main>
  )
}
