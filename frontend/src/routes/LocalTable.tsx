import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Mahgen } from '@/components/Mahgen'
import {
  loadLocalGameView,
  submitLocalGameAction,
  type LocalGameActionView,
  type LocalGameLoadResult,
  type LocalGamePlayerView,
  type LocalGameRecommendationView,
  type LocalGameView,
} from '@/lib/localGame'

export function LocalTable() {
  const { t } = useTranslation()
  const [loadResult, setLoadResult] = useState<LocalGameLoadResult | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [submittingActionId, setSubmittingActionId] = useState<number | null>(null)
  const [submitError, setSubmitError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setIsLoading(true)
    loadLocalGameView()
      .then((result) => {
        if (!cancelled) setLoadResult(result)
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  if (!loadResult) {
    return (
      <div className="min-h-full bg-background p-4 lg:p-6">
        <Card>
          <CardContent className="p-4 text-sm text-muted-foreground">
            {isLoading ? 'Loading local game view...' : 'Local game view unavailable.'}
          </CardContent>
        </Card>
      </div>
    )
  }

  const { view } = loadResult
  const self = view.players.find((player) => player.isSelf)
  const opponents = view.players.filter((player) => !player.isSelf)
  const canSubmitAction = loadResult.mode === 'tauri' && loadResult.gameId.length > 0

  async function handleSubmitAction(actionId: number) {
    if (!canSubmitAction || !loadResult) return
    const current: LocalGameLoadResult = loadResult
    setSubmittingActionId(actionId)
    setSubmitError(null)
    try {
      const nextView = await submitLocalGameAction(current.gameId, actionId)
      setLoadResult({
        gameId: current.gameId,
        view: nextView,
        mode: current.mode,
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setSubmitError(message)
    } finally {
      setSubmittingActionId(null)
    }
  }

  return (
    <div className="min-h-full bg-background p-4 lg:p-6">
      <div className="mb-4 flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <div className="mb-2 flex items-center gap-2">
            <h1 className="text-2xl font-semibold tracking-tight">{t('local_table.title')}</h1>
            <Badge variant="secondary">{view.source}</Badge>
            <Badge variant="outline">{view.phaseLabel}</Badge>
            <Badge variant={loadResult.mode === 'tauri' ? 'default' : 'destructive'}>
              {loadResult.mode === 'tauri' ? 'Tauri command' : 'Dev fallback'}
            </Badge>
          </div>
          <p className="max-w-3xl text-sm text-muted-foreground">{t('local_table.description')}</p>
          <p className="mt-1 max-w-3xl text-xs text-muted-foreground">{view.notice}</p>
          {loadResult.error && (
            <p className="mt-1 max-w-3xl text-xs text-destructive">{loadResult.error}</p>
          )}
          {submitError && <p className="mt-1 max-w-3xl text-xs text-destructive">{submitError}</p>}
        </div>
        <RoundSummary view={view} />
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_20rem]">
        <section className="grid min-h-[42rem] gap-4 rounded-lg border bg-emerald-950/20 p-4 lg:grid-rows-[auto_minmax(0,1fr)_auto]">
          <div className="grid gap-4 lg:grid-cols-3">
            {opponents.map((player) => (
              <PlayerPanel key={player.seat} player={player} />
            ))}
          </div>

          <div className="relative grid min-h-[18rem] place-items-center rounded-lg border border-emerald-800/50 bg-emerald-950/30 p-4">
            <div className="absolute inset-4 grid grid-cols-2 gap-4 opacity-80 lg:grid-cols-4">
              {view.players.map((player) => (
                <RiverPreview key={player.seat} player={player} />
              ))}
            </div>
            <div className="z-10 rounded-lg border bg-background/95 px-5 py-4 text-center shadow-sm">
              <div className="text-xs uppercase tracking-wider text-muted-foreground">
                {t('local_table.center_label')}
              </div>
              <div className="mt-2 flex items-center justify-center gap-3">
                <span className="font-mono text-lg">{view.round.roundLabel}</span>
                <span className="text-sm text-muted-foreground">
                  {t('local_table.remaining_tiles', { count: view.round.remainingTiles })}
                </span>
              </div>
            </div>
          </div>

          {self && <SelfHandPanel selfHandTiles={view.selfHandTiles} />}
        </section>

        <aside className="grid content-start gap-4">
          <ActionPanel
            actions={view.actions}
            canSubmit={canSubmitAction}
            submittingActionId={submittingActionId}
            onSubmitAction={handleSubmitAction}
          />
          <RecommendationPanel recommendations={view.recommendations} />
        </aside>
      </div>
    </div>
  )
}

function RoundSummary({ view }: { view: LocalGameView }) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardContent className="flex items-center gap-5 p-3">
        <Stat label={t('local_table.honba')} value={String(view.round.honba)} />
        <Stat label={t('local_table.kyotaku')} value={String(view.round.kyotaku)} />
        <div className="flex items-center gap-2">
          <span className="text-xs uppercase text-muted-foreground">{t('local_table.dora')}</span>
          <Mahgen seq={view.doraIndicators.join('')} kind="dora" />
        </div>
      </CardContent>
    </Card>
  )
}

function PlayerPanel({ player }: { player: LocalGamePlayerView }) {
  const { t } = useTranslation()
  return (
    <Card className="min-h-36 py-0">
      <CardHeader className="flex flex-row items-center justify-between border-b px-3 py-2">
        <CardTitle className="text-sm">{player.relationLabel}</CardTitle>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {player.isDealer && <Badge variant="secondary">{t('local_table.dealer')}</Badge>}
          <span>{player.wind}</span>
          <span className="font-mono">{player.score}</span>
        </div>
      </CardHeader>
      <CardContent className="grid gap-3 p-3">
        <TileGroup label={t('local_table.river')} seq={player.riverTiles.join('')} kind="river" riverMode />
        <div className="min-h-8">
          {player.melds.length ? (
            <div className="flex flex-wrap gap-2">
              {player.melds.map((meld, index) => (
                <Mahgen key={`${player.seat}-${index}`} seq={meld.join('')} kind="melds" />
              ))}
            </div>
          ) : (
            <span className="text-xs text-muted-foreground">{t('local_table.no_melds')}</span>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

function RiverPreview({ player }: { player: LocalGamePlayerView }) {
  return (
    <div className="flex min-h-20 flex-col justify-between rounded border border-emerald-800/40 bg-background/60 p-2">
      <span className="text-xs text-muted-foreground">{player.relationLabel}</span>
      <Mahgen seq={player.riverTiles.join('')} kind="river" riverMode />
    </div>
  )
}

function SelfHandPanel({ selfHandTiles }: { selfHandTiles: string[] }) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardHeader className="flex flex-row items-center justify-between border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.self_hand')}</CardTitle>
        <span className="text-xs text-muted-foreground">{t('local_table.self_hand_hint')}</span>
      </CardHeader>
      <CardContent className="overflow-auto p-4">
        <Mahgen seq={selfHandTiles.join('')} kind="hand" />
      </CardContent>
    </Card>
  )
}

function ActionPanel({
  actions,
  canSubmit,
  submittingActionId,
  onSubmitAction,
}: {
  actions: LocalGameActionView[]
  canSubmit: boolean
  submittingActionId: number | null
  onSubmitAction: (actionId: number) => void
}) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardHeader className="border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.actions')}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2 p-3">
        {actions.map((action) => (
          <Button
            key={action.id}
            variant="outline"
            disabled={!canSubmit || !action.enabled || submittingActionId !== null}
            className="justify-start"
            onClick={() => onSubmitAction(action.id)}
          >
            <span>{action.label}</span>
            <span className="ml-auto text-xs text-muted-foreground">
              {submittingActionId === action.id ? t('local_table.submitting_action') : action.hint}
            </span>
          </Button>
        ))}
      </CardContent>
    </Card>
  )
}

function RecommendationPanel({ recommendations }: { recommendations: LocalGameRecommendationView[] }) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardHeader className="border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.recommendations')}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-3 p-3">
        {recommendations.map((recommendation) => (
          <div key={recommendation.rank} className="rounded-md border p-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="text-sm font-medium">{recommendation.label}</span>
              <Badge variant="outline">#{recommendation.rank}</Badge>
            </div>
            {recommendation.tile && (
              <div className="mb-2">
                <Mahgen seq={recommendation.tile} kind="rec" />
              </div>
            )}
            <p className="text-xs text-muted-foreground">{recommendation.note}</p>
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

function TileGroup({
  label,
  seq,
  kind,
  riverMode,
}: {
  label: string
  seq: string
  kind: 'river' | 'hand' | 'melds' | 'dora' | 'rec'
  riverMode?: boolean
}) {
  return (
    <div className="grid gap-1">
      <span className="text-xs text-muted-foreground">{label}</span>
      <Mahgen seq={seq} kind={kind} riverMode={riverMode} />
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs uppercase text-muted-foreground">{label}</div>
      <div className="font-mono text-base font-semibold">{value}</div>
    </div>
  )
}
