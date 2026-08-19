import type { LyricsDocument, LyricsLine, LyricsTrack, PlaybackSnapshot, RuntimeStatus } from './types'

const now = Date.now()

export const mockPlayback: PlaybackSnapshot = {
  sequence: 18,
  trackKey: 'qqmusic:0039MnYb0qxYhV',
  title: '晴天',
  artist: '周杰伦',
  album: '叶惠美',
  coverUrl: null,
  sourceApp: 'QQMusic.exe',
  durationMs: 269_000,
  positionMs: 72_400,
  observedAtMs: now,
  playing: true,
  capabilities: { playPause: true, previous: true, next: true, seek: false },
  error: null,
}

const lyricRows = [
  [0, '故事的小黄花'], [4_900, '从出生那年就飘着'], [10_300, '童年的荡秋千'],
  [15_100, '随记忆一直晃到现在'], [21_500, 'Re So So Si Do Si La'],
  [27_400, 'So La Si Si Si Si La Si La So'], [34_000, '吹着前奏望着天空'],
  [40_300, '我想起花瓣试着掉落'], [47_000, '为你翘课的那一天'],
  [52_600, '花落的那一天'], [58_200, '教室的那一间'], [63_800, '我怎么看不见'],
  [69_300, '消失的下雨天'], [75_100, '我好想再淋一遍'],
  [81_400, '没想到失去的勇气我还留着'], [88_000, '好想再问一遍'],
  [93_600, '你会等待还是离开'], [100_000, '刮风这天我试过握着你手'],
  [107_200, '但偏偏雨渐渐大到我看你不见'], [115_000, '还要多久我才能在你身边'],
] as const

const translationRows = [
  [0, 'The little yellow flower from the story'],
  [4_900, 'Has drifted there since the year I was born'],
  [10_300, 'The swing from childhood'],
  [15_100, 'Keeps swaying through my memories until now'],
  [34_000, 'Listening to the intro while looking at the sky'],
  [40_300, 'I remember the petals trying to fall'],
  [69_300, 'Those rainy days that disappeared'],
  [75_100, 'I wish I could stand in the rain once more'],
  [88_000, 'I want to ask you one more time'],
  [93_600, 'Would you wait, or would you leave?'],
] as const

const romanizationRows = [
  [0, 'gu shi de xiao huang hua'],
  [4_900, 'cong chu sheng na nian jiu piao zhe'],
  [10_300, 'tong nian de dang qiu qian'],
  [15_100, 'sui ji yi yi zhi huang dao xian zai'],
  [34_000, 'chui zhe qian zou wang zhe tian kong'],
  [40_300, 'wo xiang qi hua ban shi zhe diao luo'],
  [69_300, 'xiao shi de xia yu tian'],
  [75_100, 'wo hao xiang zai lin yi bian'],
  [88_000, 'hao xiang zai wen yi bian'],
  [93_600, 'ni hui deng dai hai shi li kai'],
] as const

const createWords = (text: string, startMs: number, endMs: number) => {
  const characters = Array.from(text)
  const duration = Math.max(characters.length, endMs - startMs)
  return characters.map((character, index) => ({
    text: character,
    startMs: Math.round(startMs + (duration * index) / characters.length),
    endMs: Math.round(startMs + (duration * (index + 1)) / characters.length),
  }))
}

const createTrack = (
  rows: ReadonlyArray<readonly [number, string]>,
  withWordTiming = false,
): LyricsTrack => ({
  lines: rows.map(([startMs, text], index): LyricsLine => {
    const endMs = rows[index + 1]?.[0] ?? startMs + 5_000
    return {
      text,
      startMs,
      endMs,
      words: withWordTiming ? createWords(text, startMs, Math.max(startMs + 1, endMs - 250)) : null,
    }
  }),
})

export const mockLyrics: LyricsDocument = {
  trackKey: mockPlayback.trackKey!,
  source: 'QQMusic QRC',
  offsetMs: 0,
  original: createTrack(lyricRows, true),
  translation: createTrack(translationRows),
  romanization: createTrack(romanizationRows),
}

export const mockRuntime: RuntimeStatus = {
  role: 'host',
  connection: 'connected',
  connectionError: null,
  preferredServerAddress: null,
  serverName: '办公室播放电脑',
  serverAddress: '192.168.1.36:17636',
  allowControl: true,
  peers: [
    { id: 'peer-1', name: '设计部-PC', address: '192.168.1.52', connectedAtMs: now - 1_260_000, canControl: true },
    { id: 'peer-2', name: '会议室笔记本', address: '192.168.1.71', connectedAtMs: now - 340_000, canControl: true },
  ],
  hosts: [{ id: 'host-main', name: '办公室播放电脑', address: '192.168.1.36', port: 17636, latencyMs: 12 }],
  playback: mockPlayback,
  lyrics: mockLyrics,
}
