import { HAS_TAURI, invoke } from '@/lib/tauri'

export type LocalGameRoundView = {
  roundLabel: string
  honba: number
  kyotaku: number
  remainingTiles: number
  dealerSeat: 0 | 1 | 2 | 3
}

export type LocalGamePlayerView = {
  seat: 0 | 1 | 2 | 3
  relationLabel: string
  wind: 'E' | 'S' | 'W' | 'N'
  score: number
  riverTiles: string[]
  melds: string[][]
  statusTags: string[]
  isDealer: boolean
  isSelf: boolean
}

export type LocalGameActionView = {
  id: number
  type: string
  label: string
  hint: string
  enabled: boolean
  tile?: string | null
  mjai?: Record<string, unknown> | null
}

export type LocalGameRecommendationView = {
  rank: number
  actionId: number | null
  tile: string | null
  label: string
  source: string
  status: string
  note: string
}

export type LocalArtifactStatus = {
  saved: boolean
  replayPath: string | null
  decisionPointsPath: string | null
  errorMessage: string | null
}

export type LocalReviewKeyChoice = {
  turnIndex: number
  humanActionLabel: string
  recommendedActionLabel: string | null
  humanTile: string | null
  recommendedTile: string | null
  category: string
}

export type LocalReviewSummary = {
  schemaVersion: number
  source: string
  totalDecisions: number
  top1Matches: number
  mismatchCount: number
  keyChoices: LocalReviewKeyChoice[]
  note: string
}

export type LocalGameView = {
  schemaVersion: number
  source: string
  phaseLabel: string
  notice: string
  round: LocalGameRoundView
  players: LocalGamePlayerView[]
  selfHandTiles: string[]
  doraIndicators: string[]
  actions: LocalGameActionView[]
  recommendations: LocalGameRecommendationView[]
  artifactStatus: LocalArtifactStatus
  reviewSummary: LocalReviewSummary
}

export type LocalGameSessionHandle = {
  gameId: string
  view: LocalGameView
}

export type LocalGameLoadResult = {
  gameId: string
  view: LocalGameView
  mode: 'tauri' | 'fallback'
  error?: string
}

const DEV_FALLBACK_VIEW: LocalGameView = {
  schemaVersion: 1,
  source: 'frontend_dev_fallback',
  phaseLabel: 'Dev fallback',
  notice: 'Vite browser fallback only. Tauri desktop uses local_game_new.',
  round: {
    roundLabel: 'E1',
    honba: 0,
    kyotaku: 0,
    remainingTiles: 62,
    dealerSeat: 0,
  },
  selfHandTiles: ['123m', '405p', '678s', '11z'],
  doraIndicators: ['5m'],
  players: [
    {
      seat: 0,
      relationLabel: '自家',
      wind: 'E',
      score: 25000,
      riverTiles: ['1m', '2p', '9s'],
      melds: [],
      statusTags: ['thinking'],
      isDealer: true,
      isSelf: true,
    },
    {
      seat: 1,
      relationLabel: '下家',
      wind: 'S',
      score: 25000,
      riverTiles: ['3m', '7p', 'P'],
      melds: [['P', 'P', 'P']],
      statusTags: [],
      isDealer: false,
      isSelf: false,
    },
    {
      seat: 2,
      relationLabel: '对面',
      wind: 'W',
      score: 25000,
      riverTiles: ['9m', '2s', 'C'],
      melds: [],
      statusTags: [],
      isDealer: false,
      isSelf: false,
    },
    {
      seat: 3,
      relationLabel: '上家',
      wind: 'N',
      score: 25000,
      riverTiles: ['4m', '8p', 'F'],
      melds: [['3p', '3p', '3p']],
      statusTags: [],
      isDealer: false,
      isSelf: false,
    },
  ],
  actions: [
    {
      id: 1,
      type: 'discard',
      label: 'Discard 5p',
      hint: 'Dev fallback action',
      enabled: false,
      tile: '5p',
      mjai: { type: 'dahai', actor: 0, pai: '5p' },
    },
    { id: 2, type: 'riichi', label: 'Riichi', hint: 'Not wired yet', enabled: false },
    { id: 3, type: 'skip', label: 'Skip', hint: 'Not wired yet', enabled: false },
  ],
  recommendations: [
    {
      rank: 1,
      actionId: 1,
      tile: '5p',
      label: 'Recommended discard 5p',
      source: 'frontend_dev_fallback',
      status: 'recommended',
      note: 'Dev fallback only; not a real AI recommendation.',
    },
  ],
  artifactStatus: {
    saved: false,
    replayPath: null,
    decisionPointsPath: null,
    errorMessage: null,
  },
  reviewSummary: {
    schemaVersion: 1,
    source: 'frontend_dev_fallback',
    totalDecisions: 0,
    top1Matches: 0,
    mismatchCount: 0,
    keyChoices: [],
    note: 'Dev fallback only; no local review summary.',
  },
}

export async function loadLocalGameView(): Promise<LocalGameLoadResult> {
  if (!HAS_TAURI) {
    return { gameId: 'frontend-dev-fallback', view: DEV_FALLBACK_VIEW, mode: 'fallback' }
  }

  try {
    const session = await invoke<LocalGameSessionHandle>('local_game_new')
    return { gameId: session.gameId, view: session.view, mode: 'tauri' }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    return {
      gameId: 'frontend-dev-fallback',
      view: {
        ...DEV_FALLBACK_VIEW,
        notice: `Tauri command failed; showing dev fallback. ${message}`,
      },
      mode: 'fallback',
      error: message,
    }
  }
}

export async function getLocalGameView(gameId: string): Promise<LocalGameView> {
  return await invoke<LocalGameView>('local_game_get_view', { gameId })
}

export async function submitLocalGameAction(gameId: string, actionId: number): Promise<LocalGameView> {
  return await invoke<LocalGameView>('local_game_submit_action', { gameId, actionId })
}
