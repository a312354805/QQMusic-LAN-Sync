import {
  AlignCenter,
  AlignLeft,
  ArrowLeft,
  ArrowRight,
  LocateFixed,
  Minus,
  Music2,
  Plus,
} from 'lucide-react'
import clsx from 'clsx'
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from 'react'
import type { LyricsDocument, LyricsLine, LyricsTrack } from '../shared/types'
import { IconButton } from './IconButton'
import './LyricsPanel.scss'

type LyricsPanelProps = {
  document: LyricsDocument | null
  positionMs: number
  title: string | null
  artist: string | null
  mode?: 'compact' | 'focus'
}

type LyricsPreferences = {
  fontSize: number
  alignment: 'left' | 'center'
  showTranslation: boolean
  showRomanization: boolean
  offsetAdjustments: Record<string, number>
}

const preferencesKey = 'qqmusic-lan-sync:lyrics-display'
const defaultPreferences: LyricsPreferences = {
  fontSize: 30,
  alignment: 'left',
  showTranslation: true,
  showRomanization: true,
  offsetAdjustments: {},
}

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.min(maximum, Math.max(minimum, value))

const loadPreferences = (): LyricsPreferences => {
  try {
    const saved = window.localStorage.getItem(preferencesKey)
    if (!saved) return defaultPreferences
    const value = JSON.parse(saved) as Partial<LyricsPreferences>
    return {
      fontSize: clamp(Number(value.fontSize) || defaultPreferences.fontSize, 16, 40),
      alignment: value.alignment === 'center' ? 'center' : 'left',
      showTranslation: value.showTranslation ?? true,
      showRomanization: value.showRomanization ?? true,
      offsetAdjustments: value.offsetAdjustments ?? {},
    }
  } catch {
    return defaultPreferences
  }
}

const formatOffset = (milliseconds: number) => {
  if (milliseconds === 0) return '0 ms'
  if (Math.abs(milliseconds) >= 1000 && milliseconds % 1000 === 0) {
    return `${milliseconds > 0 ? '+' : ''}${milliseconds / 1000} s`
  }
  return `${milliseconds > 0 ? '+' : ''}${milliseconds} ms`
}

const findAlignedLine = (track: LyricsTrack | null, line: LyricsLine) => {
  if (!track) return null
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

function TimedLine({ line, active, adjustedPositionMs }: {
  line: LyricsLine
  active: boolean
  adjustedPositionMs: number
}) {
  if (!active || !line.words?.length) return <p>{line.text || '\u00a0'}</p>

  return (
    <p className="lyrics-panel__timed-text" aria-label={line.text}>
      {line.words.map((word, index) => {
        const duration = Math.max(1, word.endMs - word.startMs)
        const progress = clamp((adjustedPositionMs - word.startMs) / duration, 0, 1)
        return (
          <span
            aria-hidden="true"
            className="lyrics-panel__word"
            key={`${word.startMs}-${index}`}
            style={{ '--word-progress': `${progress * 100}%` } as CSSProperties}
          >
            {word.text}
          </span>
        )
      })}
    </p>
  )
}

export function LyricsPanel({
  document,
  positionMs,
  title,
  artist,
  mode = 'compact',
}: LyricsPanelProps) {
  const [preferences, setPreferences] = useState(loadPreferences)
  const [following, setFollowing] = useState(true)
  const scrollerRef = useRef<HTMLDivElement>(null)
  const activeLineRef = useRef<HTMLDivElement>(null)
  const lines = useMemo(() => document?.original.lines ?? [], [document])
  const trackKey = document?.trackKey ?? null
  const localOffsetMs = trackKey ? preferences.offsetAdjustments[trackKey] ?? 0 : 0
  const effectiveOffsetMs = (document?.offsetMs ?? 0) + localOffsetMs
  const adjustedPositionMs = positionMs + effectiveOffsetMs
  const translationAvailable = Boolean(document?.translation?.lines.length)
  const romanizationAvailable = Boolean(document?.romanization?.lines.length)
  const wordTimed = lines.some((line) => Boolean(line.words?.length))

  const activeIndex = useMemo(() => {
    let found = -1
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].startMs > adjustedPositionMs) break
      found = index
    }
    return found
  }, [adjustedPositionMs, lines])

  const auxiliaryLines = useMemo(() => lines.map((line) => ({
    translation: preferences.showTranslation
      ? findAlignedLine(document?.translation ?? null, line)
      : null,
    romanization: preferences.showRomanization
      ? findAlignedLine(document?.romanization ?? null, line)
      : null,
  })), [document, lines, preferences.showRomanization, preferences.showTranslation])

  useEffect(() => {
    try {
      window.localStorage.setItem(preferencesKey, JSON.stringify(preferences))
    } catch {
      // Preferences are optional; lyrics remain usable when storage is unavailable.
    }
  }, [preferences])

  useEffect(() => setFollowing(true), [trackKey])

  useEffect(() => {
    if (!following || !scrollerRef.current || !activeLineRef.current) return
    const scroller = scrollerRef.current
    const activeLine = activeLineRef.current
    const top = activeLine.offsetTop - (scroller.clientHeight - activeLine.offsetHeight) / 2
    scroller.scrollTo({
      top: Math.max(0, top),
      behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth',
    })
  }, [activeIndex, following, mode])

  const patchPreferences = (patch: Partial<LyricsPreferences>) => {
    setPreferences((current) => ({ ...current, ...patch }))
  }

  const changeOffset = (deltaMs: number) => {
    if (!trackKey) return
    setPreferences((current) => ({
      ...current,
      offsetAdjustments: {
        ...current.offsetAdjustments,
        [trackKey]: clamp((current.offsetAdjustments[trackKey] ?? 0) + deltaMs, -10_000, 10_000),
      },
    }))
  }

  const resetOffset = () => {
    if (!trackKey) return
    setPreferences((current) => ({
      ...current,
      offsetAdjustments: { ...current.offsetAdjustments, [trackKey]: 0 },
    }))
  }

  const pauseFollowingFromKeyboard = (event: KeyboardEvent<HTMLDivElement>) => {
    if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End'].includes(event.key)) {
      setFollowing(false)
    }
  }

  const resumeFollowing = () => {
    setFollowing(true)
    window.requestAnimationFrame(() => {
      const scroller = scrollerRef.current
      const activeLine = activeLineRef.current
      if (!scroller || !activeLine) return
      const top = activeLine.offsetTop - (scroller.clientHeight - activeLine.offsetHeight) / 2
      scroller.scrollTo({
        top: Math.max(0, top),
        behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth',
      })
    })
  }

  const compactFontSize = clamp(Math.round(preferences.fontSize * 0.76), 16, 25)
  const secondaryFontSize = Math.max(12, Math.round(preferences.fontSize * 0.48))
  const compactSecondaryFontSize = Math.max(10, Math.round(compactFontSize * 0.58))
  const currentLine = lines[activeIndex]

  return (
    <section
      className={clsx('lyrics-panel', `lyrics-panel--${mode}`)}
      aria-label="同步歌词"
      style={{
        '--lyrics-font-size': `${preferences.fontSize}px`,
        '--lyrics-compact-font-size': `${compactFontSize}px`,
        '--lyrics-secondary-font-size': `${secondaryFontSize}px`,
        '--lyrics-compact-secondary-font-size': `${compactSecondaryFontSize}px`,
        '--lyrics-text-align': preferences.alignment,
      } as CSSProperties}
    >
      <header className="lyrics-panel__header">
        <div className="lyrics-panel__track">
          <strong>{title ?? '等待 QQ 音乐'}</strong>
          <span>{artist ?? '尚未检测到正在播放的歌曲'}</span>
        </div>
        <div className="lyrics-panel__status" aria-label="歌词状态">
          <span>{document?.source ?? '等待歌词'}</span>
          {wordTimed && <em>逐字</em>}
        </div>
        <div className="lyrics-panel__toolbar" role="toolbar" aria-label="歌词显示工具">
          <div className="lyrics-tool-group" role="group" aria-label="字体大小">
            <IconButton label="缩小歌词" size="sm" disabled={preferences.fontSize <= 16} onClick={() => patchPreferences({ fontSize: Math.max(16, preferences.fontSize - 2) })}><Minus size={15} /></IconButton>
            <span className="lyrics-tool-value" aria-hidden="true">{preferences.fontSize}</span>
            <IconButton label="放大歌词" size="sm" disabled={preferences.fontSize >= 40} onClick={() => patchPreferences({ fontSize: Math.min(40, preferences.fontSize + 2) })}><Plus size={15} /></IconButton>
          </div>
          <div className="lyrics-tool-group" role="group" aria-label="歌词对齐">
            <IconButton label="歌词左对齐" size="sm" aria-pressed={preferences.alignment === 'left'} className={clsx(preferences.alignment === 'left' && 'is-on')} onClick={() => patchPreferences({ alignment: 'left' })}><AlignLeft size={15} /></IconButton>
            <IconButton label="歌词居中对齐" size="sm" aria-pressed={preferences.alignment === 'center'} className={clsx(preferences.alignment === 'center' && 'is-on')} onClick={() => patchPreferences({ alignment: 'center' })}><AlignCenter size={15} /></IconButton>
          </div>
          <div className="lyrics-tool-group" role="group" aria-label="辅助歌词">
            <IconButton label={translationAvailable ? '显示或隐藏翻译' : '当前歌词没有翻译'} size="sm" disabled={!translationAvailable} aria-pressed={preferences.showTranslation} className={clsx(preferences.showTranslation && translationAvailable && 'is-on')} onClick={() => patchPreferences({ showTranslation: !preferences.showTranslation })}><span className="lyrics-tool-glyph">译</span></IconButton>
            <IconButton label={romanizationAvailable ? '显示或隐藏罗马音' : '当前歌词没有罗马音'} size="sm" disabled={!romanizationAvailable} aria-pressed={preferences.showRomanization} className={clsx(preferences.showRomanization && romanizationAvailable && 'is-on')} onClick={() => patchPreferences({ showRomanization: !preferences.showRomanization })}><span className="lyrics-tool-glyph">音</span></IconButton>
          </div>
          <div className="lyrics-tool-group lyrics-offset" role="group" aria-label={`歌词偏移 ${formatOffset(effectiveOffsetMs)}`}>
            <IconButton label="歌词延后 100 毫秒" size="sm" disabled={!document} onClick={() => changeOffset(-100)}><ArrowLeft size={15} /></IconButton>
            <button type="button" className="lyrics-offset__value" disabled={!document || localOffsetMs === 0} title="重置本地歌词微调" onClick={resetOffset}>{formatOffset(effectiveOffsetMs)}</button>
            <IconButton label="歌词提前 100 毫秒" size="sm" disabled={!document} onClick={() => changeOffset(100)}><ArrowRight size={15} /></IconButton>
          </div>
        </div>
      </header>

      {lines.length > 0 ? (
        <div className="lyrics-panel__workspace">
          <div
            className="lyrics-panel__scroller"
            ref={scrollerRef}
            tabIndex={0}
            onWheel={() => setFollowing(false)}
            onPointerDown={() => setFollowing(false)}
            onKeyDown={pauseFollowingFromKeyboard}
          >
            <div className="lyrics-panel__lines" role="list">
              {lines.map((line, index) => {
                const active = index === activeIndex
                const past = index < activeIndex
                const supporting = auxiliaryLines[index]
                return (
                  <div
                    className={clsx('lyrics-panel__line', active && 'is-active', past && 'is-past')}
                    key={`${line.startMs}-${index}`}
                    ref={active ? activeLineRef : undefined}
                    role="listitem"
                    aria-current={active ? 'true' : undefined}
                  >
                    <TimedLine line={line} active={active} adjustedPositionMs={adjustedPositionMs} />
                    {supporting.translation && <small data-kind="translation">{supporting.translation.text}</small>}
                    {supporting.romanization && <small data-kind="romanization">{supporting.romanization.text}</small>}
                  </div>
                )
              })}
            </div>
          </div>
          {!following && (
            <button type="button" className="lyrics-follow-button" onClick={resumeFollowing}>
              <LocateFixed size={16} />回到当前歌词
            </button>
          )}
          <div className="sr-only" aria-live="polite" aria-atomic="true">
            {currentLine?.text ?? ''}
          </div>
        </div>
      ) : (
        <div className="lyrics-panel__empty" role="status">
          <span><Music2 size={22} /></span>
          <strong>{title ? '正在获取同步歌词' : '等待 QQ 音乐播放歌曲'}</strong>
          <small>{title ? '歌词返回后会自动开始跟随' : '主机播放歌曲后，这里会自动更新'}</small>
        </div>
      )}
    </section>
  )
}
