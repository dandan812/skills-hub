import { useCallback, useEffect, useState } from 'react'
import {
  Activity,
  AlertTriangle,
  Check,
  ChevronDown,
  CircleHelp,
  Database,
  Inbox,
  LoaderCircle,
  Minus,
  RefreshCw,
  Search,
} from 'lucide-react'
import type { TFunction } from 'i18next'
import {
  filterRuntimeRows,
  findAgent,
  findSession,
  sessionKey,
} from './status'
import type {
  RuntimeAgentId,
  RuntimeEvidenceInvoke,
  RuntimeEvidenceOverview,
  RuntimeEvidenceSource,
  TruthState,
} from './types'

type RuntimeEvidencePageProps = {
  isTauri: boolean
  invokeTauri: RuntimeEvidenceInvoke
  t: TFunction
}

type LoadState = 'loading' | 'ready' | 'refreshing' | 'error' | 'unavailable'

const AGENTS: RuntimeAgentId[] = ['codex', 'claude', 'opencode', 'pi']

const formatObservedAt = (value: string): string => {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}

const TruthBadge = ({ value, t }: { value: TruthState; t: TFunction }) => (
  <span className={`runtime-truth runtime-truth-${value}`}>
    {value === 'yes' ? <Check size={13} /> : value === 'no' ? <Minus size={13} /> : <CircleHelp size={13} />}
    {t(`runtimeEvidence.truth.${value}`)}
  </span>
)

const RuntimeEvidencePage = ({
  isTauri,
  invokeTauri,
  t,
}: RuntimeEvidencePageProps) => {
  const [loadState, setLoadState] = useState<LoadState>(
    isTauri ? 'loading' : 'unavailable',
  )
  const [overview, setOverview] = useState<RuntimeEvidenceOverview | null>(null)
  const [selectedAgent, setSelectedAgent] = useState<RuntimeAgentId>('codex')
  const [selectedSession, setSelectedSession] = useState('')
  const [query, setQuery] = useState('')
  const [error, setError] = useState('')

  const requestOverview = useCallback(
    (command: 'get_runtime_evidence_overview' | 'refresh_runtime_evidence') =>
      invokeTauri<RuntimeEvidenceOverview>(command),
    [invokeTauri],
  )

  useEffect(() => {
    if (!isTauri) return
    let active = true
    void requestOverview('get_runtime_evidence_overview')
      .then((nextOverview) => {
        if (!active) return
        setOverview(nextOverview)
        setLoadState('ready')
      })
      .catch((cause) => {
        if (!active) return
        setError(cause instanceof Error ? cause.message : String(cause))
        setLoadState('error')
      })
    return () => {
      active = false
    }
  }, [isTauri, requestOverview])

  const agent = findAgent(overview, selectedAgent)
  const session = findSession(agent, selectedSession)
  const rows = filterRuntimeRows(session?.skills ?? [], query)

  const refresh = useCallback(async () => {
    setLoadState('refreshing')
    setError('')
    try {
      const nextOverview = await requestOverview('refresh_runtime_evidence')
      setOverview(nextOverview)
      setLoadState('ready')
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
      setLoadState('error')
    }
  }, [requestOverview])

  const renderSource = (source: RuntimeEvidenceSource) =>
    t(`runtimeEvidence.sources.${source}`)

  return (
    <section className="runtime-evidence-page">
      <header className="runtime-evidence-header">
        <div>
          <h1>{t('runtimeEvidence.title')}</h1>
          <p>{t('runtimeEvidence.subtitle')}</p>
        </div>
        <button
          className="btn btn-primary"
          type="button"
          onClick={() => void refresh()}
          disabled={loadState === 'loading' || loadState === 'refreshing' || !isTauri}
        >
          {loadState === 'refreshing' ? (
            <LoaderCircle className="runtime-evidence-spin" size={15} />
          ) : (
            <RefreshCw size={15} />
          )}
          {t('runtimeEvidence.refresh')}
        </button>
      </header>

      <div className="runtime-evidence-body">
        {loadState === 'loading' ? (
          <div className="runtime-evidence-message" role="status">
            <LoaderCircle className="runtime-evidence-spin" size={18} />
            <span>{t('runtimeEvidence.loading')}</span>
          </div>
        ) : loadState === 'unavailable' ? (
          <div className="runtime-evidence-message warning" role="status">
            <AlertTriangle size={18} />
            <span>{t('runtimeEvidence.unavailable')}</span>
          </div>
        ) : !overview ? (
          <div className="runtime-evidence-message error" role="alert">
            <AlertTriangle size={18} />
            <div>
              <strong>{t('runtimeEvidence.error')}</strong>
              <span>{error}</span>
            </div>
          </div>
        ) : (
          <>
            {error && (
              <div className="runtime-evidence-message error compact" role="alert">
                <AlertTriangle size={16} />
                <span>{error}</span>
              </div>
            )}

            <div className="runtime-evidence-summary" aria-label={t('runtimeEvidence.summary')}>
              <div>
                <Activity size={16} />
                <span>{t('runtimeEvidence.events')}</span>
                <strong>{overview.eventCount}</strong>
              </div>
              <div>
                <Database size={16} />
                <span>{t('runtimeEvidence.imported')}</span>
                <strong>{overview.import.accepted}</strong>
              </div>
              <div className={overview.import.rejected > 0 ? 'warning' : ''}>
                <AlertTriangle size={16} />
                <span>{t('runtimeEvidence.rejected')}</span>
                <strong>{overview.import.rejected}</strong>
              </div>
              <div>
                <span className={`runtime-overview-dot ${overview.status}`} />
                <span>{t('runtimeEvidence.dataState')}</span>
                <strong>{t(`runtimeEvidence.overviewStates.${overview.status}`)}</strong>
              </div>
            </div>

            {overview.import.rejected > 0 && (
              <div className="runtime-import-warning" title={overview.import.reasons.join(', ')}>
                <AlertTriangle size={15} />
                <span>{t('runtimeEvidence.rejectedDetail', { count: overview.import.rejected })}</span>
              </div>
            )}

            <div className="runtime-agent-tabs" role="group" aria-label={t('runtimeEvidence.agent')}>
              {AGENTS.map((agentId) => (
                <button
                  key={agentId}
                  type="button"
                  className={selectedAgent === agentId ? 'active' : ''}
                  aria-pressed={selectedAgent === agentId}
                  onClick={() => {
                    setSelectedAgent(agentId)
                    setSelectedSession('')
                  }}
                >
                  {t(`runtimeEvidence.agents.${agentId}`)}
                </button>
              ))}
            </div>

            <div className="runtime-evidence-toolbar">
              <label className="runtime-session-field">
                <span>{t('runtimeEvidence.session')}</span>
                <div className="runtime-select-wrap">
                  <select
                    value={session ? sessionKey(session) : ''}
                    onChange={(event) => setSelectedSession(event.target.value)}
                    disabled={!agent?.sessions.length}
                  >
                    {agent?.sessions.map((candidate) => (
                      <option key={sessionKey(candidate)} value={sessionKey(candidate)}>
                        {candidate.sessionId ?? t('runtimeEvidence.noSession')}
                        {' · '}
                        {t(`runtimeEvidence.sessionStates.${candidate.state}`)}
                      </option>
                    ))}
                  </select>
                  <ChevronDown size={14} />
                </div>
              </label>
              <label className="runtime-search-field">
                <span className="sr-only">{t('runtimeEvidence.search')}</span>
                <Search size={15} />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t('runtimeEvidence.search')}
                />
              </label>
            </div>

            <div className="runtime-table-wrap">
              <table className="runtime-table">
                <thead>
                  <tr>
                    <th>{t('runtimeEvidence.columns.skill')}</th>
                    <th>{t('runtimeEvidence.columns.installed')}</th>
                    <th>{t('runtimeEvidence.columns.assigned')}</th>
                    <th>{t('runtimeEvidence.columns.loaded')}</th>
                    <th>{t('runtimeEvidence.columns.lastCall')}</th>
                    <th>{t('runtimeEvidence.columns.source')}</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((row) => (
                    <tr key={row.skillId}>
                      <td>
                        <strong>{row.skillName}</strong>
                        {row.skillName !== row.skillId && <code>{row.skillId}</code>}
                      </td>
                      <td><TruthBadge value={row.installed} t={t} /></td>
                      <td><TruthBadge value={row.assigned} t={t} /></td>
                      <td><TruthBadge value={row.loaded} t={t} /></td>
                      <td>
                        {row.lastCall.state === 'observed' ? (
                          <time dateTime={row.lastCall.observedAt}>
                            {formatObservedAt(row.lastCall.observedAt)}
                          </time>
                        ) : (
                          <span className="runtime-unknown">{t('runtimeEvidence.truth.unknown')}</span>
                        )}
                      </td>
                      <td>
                        <span className={`runtime-source runtime-source-${row.source}`}>
                          {renderSource(row.source)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {rows.length === 0 && (
                <div className="runtime-empty">
                  <CircleHelp size={18} />
                  <span>{query ? t('runtimeEvidence.noResults') : t('runtimeEvidence.noSkills')}</span>
                </div>
              )}
            </div>

            <div className="runtime-inbox-row">
              <Inbox size={14} />
              <span>{t('runtimeEvidence.inbox')}</span>
              <code>{overview.inboxPath ?? t('runtimeEvidence.unavailableValue')}</code>
            </div>
          </>
        )}
      </div>
    </section>
  )
}

export default RuntimeEvidencePage
