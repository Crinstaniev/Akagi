import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Mahgen } from '@/components/Mahgen'
import { mjaiToMahgen } from '@/lib/tileIdx'
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
  const primaryRecommendation =
    view.recommendations.find((recommendation) => recommendation.rank === 1) ??
    view.recommendations[0]
  const recommendedActionId =
    primaryRecommendation?.status === 'recommended' ? primaryRecommendation.actionId : null
  const recommendedDiscardTile =
    primaryRecommendation?.status === 'recommended' &&
    view.actions.some(
      (action) =>
        action.id === primaryRecommendation.actionId &&
        normalizeActionType(action.type) === 'discard',
    )
      ? primaryRecommendation.tile
      : null

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

          {self && (
            <SelfHandPanel
              selfHandTiles={view.selfHandTiles}
              recommendedDiscardTile={recommendedDiscardTile}
            />
          )}
        </section>

        <aside className="grid content-start gap-4">
          <EnginePanel view={view} />
          <ActionPanel
            actions={view.actions}
            canSubmit={canSubmitAction}
            submittingActionId={submittingActionId}
            recommendedActionId={recommendedActionId}
            onSubmitAction={handleSubmitAction}
          />
          <RecommendationPanel recommendations={view.recommendations} />
          <ArtifactPanel view={view} />
          <ReviewSummaryPanel view={view} />
        </aside>
      </div>
    </div>
  )
}

function EnginePanel({ view }: { view: LocalGameView }) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardHeader className="border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.engine')}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2 p-3 text-xs">
        <div className="flex items-center justify-between gap-3">
          <span className="text-muted-foreground">{t('local_table.engine_source')}</span>
          <Badge variant="secondary">{view.engine.source}</Badge>
        </div>
        <div className="flex items-center justify-between gap-3">
          <span className="text-muted-foreground">{t('local_table.engine_status')}</span>
          <Badge variant={view.engine.status === 'terminal' ? 'outline' : 'default'}>
            {view.engine.status}
          </Badge>
        </div>
        <div className="flex items-center justify-between gap-3">
          <span className="text-muted-foreground">{t('local_table.ai_worker')}</span>
          <Badge variant={view.engine.worker.configured ? 'default' : 'outline'}>
            {view.engine.worker.label}
          </Badge>
        </div>
        {view.engine.worker.timeoutMs ? (
          <div className="flex items-center justify-between gap-3">
            <span className="text-muted-foreground">{t('local_table.worker_timeout')}</span>
            <span>{view.engine.worker.timeoutMs} ms</span>
          </div>
        ) : null}
        <div className="flex flex-wrap gap-1">
          {view.engine.capabilities.map((capability) => (
            <Badge key={capability} variant="outline">
              {capability}
            </Badge>
          ))}
        </div>
        <p className="text-muted-foreground">{view.engine.note}</p>
      </CardContent>
    </Card>
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
          <SafeTiles tiles={view.doraIndicators} kind="dora" />
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
        <TileGroup label={t('local_table.river')} tiles={player.riverTiles} kind="river" riverMode />
        <div className="min-h-8">
          {player.melds.length ? (
            <div className="flex flex-wrap gap-2">
              {player.melds.map((meld, index) => (
                <SafeTiles key={`${player.seat}-${index}`} tiles={meld} kind="melds" />
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
      <SafeTiles tiles={player.riverTiles} kind="river" riverMode />
    </div>
  )
}

function SelfHandPanel({
  selfHandTiles,
  recommendedDiscardTile,
}: {
  selfHandTiles: string[]
  recommendedDiscardTile: string | null
}) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardHeader className="flex flex-row items-center justify-between border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.self_hand')}</CardTitle>
        <span className="text-xs text-muted-foreground">{t('local_table.self_hand_hint')}</span>
      </CardHeader>
      <CardContent className="overflow-auto p-4">
        <SafeTiles tiles={selfHandTiles} kind="hand" highlightTile={recommendedDiscardTile} />
      </CardContent>
    </Card>
  )
}

function ActionPanel({
  actions,
  canSubmit,
  submittingActionId,
  recommendedActionId,
  onSubmitAction,
}: {
  actions: LocalGameActionView[]
  canSubmit: boolean
  submittingActionId: number | null
  recommendedActionId: number | null
  onSubmitAction: (actionId: number) => void
}) {
  const { t } = useTranslation()
  return (
    <Card className="py-0">
      <CardHeader className="border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.actions')}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2 p-3">
        {actions.map((action) => {
          const isRecommended = action.id === recommendedActionId
          return (
            <Button
              key={action.id}
              variant={isRecommended ? 'default' : 'outline'}
              disabled={!canSubmit || !action.enabled || submittingActionId !== null}
              className={`h-auto justify-start gap-3 py-2 ${
                isRecommended ? 'ring-2 ring-primary/45 ring-offset-1' : ''
              }`}
              onClick={() => onSubmitAction(action.id)}
            >
              <Badge variant={action.type === 'discard' ? 'secondary' : 'outline'}>
                {actionTypeLabel(t, action.type)}
              </Badge>
              {action.tile && (
                <span className="shrink-0">
                  <SafeTiles tiles={[action.tile]} kind="rec" fallback={action.tile} />
                </span>
              )}
              <span className="min-w-0 flex-1 truncate text-left">{action.label}</span>
              <span className="max-w-32 truncate text-xs text-muted-foreground">
                {submittingActionId === action.id
                  ? t('local_table.submitting_action')
                  : isRecommended
                    ? t('local_table.recommended_action')
                    : action.hint}
              </span>
            </Button>
          )
        })}
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
              <div className="flex items-center gap-1">
                <Badge
                  variant={recommendation.status === 'recommended' ? 'default' : 'destructive'}
                >
                  {recommendationStatusLabel(t, recommendation.status)}
                </Badge>
                <Badge variant="outline">#{recommendation.rank}</Badge>
              </div>
            </div>
            <div className="mb-2 flex flex-wrap items-center gap-1 text-xs">
              <Badge variant="secondary">{recommendation.source}</Badge>
              {recommendation.elapsedMs != null && (
                <span className="text-muted-foreground">
                  {t('local_table.recommendation_elapsed', {
                    count: recommendation.elapsedMs.toFixed(1),
                  })}
                </span>
              )}
            </div>
            {recommendation.tile && (
              <div className="mb-2">
                <SafeTiles tiles={[recommendation.tile]} kind="rec" fallback={recommendation.tile} />
              </div>
            )}
            <p className="text-xs text-muted-foreground">
              {recommendation.reason ?? recommendation.note}
            </p>
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

function ArtifactPanel({ view }: { view: LocalGameView }) {
  const { t } = useTranslation()
  const { artifactStatus } = view
  const isTerminal = view.actions.length === 0 || view.phaseLabel === 'Exhaustive draw'
  return (
    <Card className="py-0">
      <CardHeader className="border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.artifacts')}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2 p-3 text-xs">
        <div className="flex items-center justify-between gap-3">
          <span className="text-muted-foreground">{t('local_table.artifact_status')}</span>
          <Badge
            variant={artifactStatus.saved ? 'default' : artifactStatus.errorMessage ? 'destructive' : 'outline'}
          >
            {artifactStatus.saved
              ? t('local_table.artifact_saved')
              : artifactStatus.errorMessage
                ? t('local_table.artifact_failed')
                : isTerminal
                  ? t('local_table.artifact_pending')
                  : t('local_table.artifact_waiting')}
          </Badge>
        </div>
        {artifactStatus.errorMessage && (
          <p className="break-words text-destructive">{artifactStatus.errorMessage}</p>
        )}
        {artifactStatus.replayPath && (
          <PathLine label={t('local_table.replay_path')} value={artifactStatus.replayPath} />
        )}
        {artifactStatus.decisionPointsPath && (
          <PathLine
            label={t('local_table.decision_points_path')}
            value={artifactStatus.decisionPointsPath}
          />
        )}
      </CardContent>
    </Card>
  )
}

function PathLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1">
      <span className="text-muted-foreground">{label}</span>
      <code className="break-all rounded border bg-muted px-2 py-1 font-mono text-[11px]">{value}</code>
    </div>
  )
}

function ReviewSummaryPanel({ view }: { view: LocalGameView }) {
  const { t } = useTranslation()
  const summary = view.reviewSummary
  return (
    <Card className="py-0">
      <CardHeader className="border-b px-3 py-2">
        <CardTitle className="text-sm">{t('local_table.review_summary')}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-3 p-3">
        <div className="grid grid-cols-3 gap-2">
          <Stat label={t('local_table.review_total')} value={String(summary.totalDecisions)} />
          <Stat label={t('local_table.review_top1')} value={String(summary.top1Matches)} />
          <Stat label={t('local_table.review_focus')} value={String(summary.attentionCount)} />
          <Stat label={t('local_table.review_fallback')} value={String(summary.fallbackCount)} />
          <Stat
            label={t('local_table.review_unavailable_count')}
            value={String(summary.unavailableCount)}
          />
          <Stat
            label={t('local_table.review_not_ranked')}
            value={String(summary.notRankedCount)}
          />
        </div>
        <p className="text-xs text-muted-foreground">{summary.note}</p>
        {summary.keyChoices.length > 0 ? (
          <div className="grid gap-2">
            {summary.keyChoices.slice(0, 5).map((choice) => (
              <div key={choice.turnIndex} className="rounded-md border p-2 text-xs">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <span className="font-medium">
                    {t('local_table.review_turn', { count: choice.turnIndex + 1 })}
                  </span>
                  <Badge variant="outline">{choice.category}</Badge>
                </div>
                <div className="grid gap-1 text-muted-foreground">
                  <span>
                    {t('local_table.review_human')}: {choice.humanActionLabel}
                  </span>
                  <span>
                    {t('local_table.review_recommended')}:{' '}
                    {choice.recommendedActionLabel ?? t('local_table.review_unavailable')}
                  </span>
                  {choice.reason && (
                    <span className="break-words">
                      {t('local_table.review_reason')}: {choice.reason}
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="rounded-md border border-dashed p-2 text-xs text-muted-foreground">
            {t('local_table.review_no_key_choices')}
          </p>
        )}
      </CardContent>
    </Card>
  )
}

function TileGroup({
  label,
  tiles,
  kind,
  riverMode,
}: {
  label: string
  tiles: string[]
  kind: 'river' | 'hand' | 'melds' | 'dora' | 'rec'
  riverMode?: boolean
}) {
  return (
    <div className="grid gap-1">
      <span className="text-xs text-muted-foreground">{label}</span>
      <SafeTiles tiles={tiles} kind={kind} riverMode={riverMode} />
    </div>
  )
}

function SafeTiles({
  tiles,
  kind,
  riverMode,
  fallback,
  highlightTile,
}: {
  tiles: string[]
  kind: 'river' | 'hand' | 'melds' | 'dora' | 'rec'
  riverMode?: boolean
  fallback?: string
  highlightTile?: string | null
}) {
  const seq = mjaiToMahgen(tiles)
  if (seq && !highlightTile) return <Mahgen seq={seq} kind={kind} riverMode={riverMode} />
  if (seq && highlightTile) {
    return (
      <span className="inline-flex max-w-full flex-wrap gap-1 align-middle">
        {tiles.map((tile, index) => {
          const itemSeq = mjaiToMahgen([tile])
          const highlighted = tile === highlightTile
          return (
            <span
              key={`${tile}-${index}`}
              className={
                highlighted
                  ? 'rounded-md bg-primary/15 p-1 ring-2 ring-primary/60 ring-offset-1'
                  : 'rounded-md p-1'
              }
            >
              {itemSeq ? <Mahgen seq={itemSeq} kind={kind} riverMode={riverMode} /> : tile}
            </span>
          )
        })}
      </span>
    )
  }

  const text = fallback ?? tiles.filter(Boolean).join(' ')
  if (!text) return <span className="text-xs text-muted-foreground">-</span>

  return (
    <span className="inline-flex max-w-full flex-wrap gap-1 align-middle">
      {text.split(/\s+/).map((part, index) => (
        <Badge
          key={`${part}-${index}`}
          variant={part === highlightTile ? 'default' : 'outline'}
          className="font-mono text-[11px]"
        >
          {part}
        </Badge>
      ))}
    </span>
  )
}

function recommendationStatusLabel(t: ReturnType<typeof useTranslation>['t'], status: string) {
  return t(`local_table.recommendation_status_${status}`, { defaultValue: status })
}

function actionTypeLabel(t: ReturnType<typeof useTranslation>['t'], actionType: string) {
  const normalized = normalizeActionType(actionType)
  return t(`local_table.action_${normalized}`, { defaultValue: actionType })
}

function normalizeActionType(actionType: string) {
  const value = actionType.toLowerCase()
  if (value === 'dahai') return 'discard'
  if (value === 'reach') return 'riichi'
  if (value === 'skip' || value === 'none') return 'pass'
  if (value.includes('kan')) return 'kan'
  return value
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs uppercase text-muted-foreground">{label}</div>
      <div className="font-mono text-base font-semibold">{value}</div>
    </div>
  )
}
