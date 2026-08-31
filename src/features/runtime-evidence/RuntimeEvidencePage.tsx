import { useCallback, useEffect, useState } from 'react'
import { Activity, CircleAlert, LoaderCircle, RefreshCw } from 'lucide-react'
import type { TFunction } from 'i18next'
import { hasObservedRuntimeEvidence } from './status'
import type {
  RuntimeEvidenceInvoke,
  RuntimeEvidenceStatus,
} from './types'

type RuntimeEvidencePageProps = {
  isTauri: boolean
  invokeTauri: RuntimeEvidenceInvoke
  t: TFunction
}

type LoadState = 'loading' | 'ready' | 'error' | 'unavailable'

const RuntimeEvidencePage = ({
  isTauri,
  invokeTauri,
  t,
}: RuntimeEvidencePageProps) => {
  const [loadState, setLoadState] = useState<LoadState>(
    isTauri ? 'loading' : 'unavailable',
  )
  const [status, setStatus] = useState<RuntimeEvidenceStatus | null>(null)
  const [error, setError] = useState('')

  const requestStatus = useCallback(
    () => invokeTauri<RuntimeEvidenceStatus>('get_runtime_evidence_status'),
    [invokeTauri],
  )

  const loadStatus = useCallback(async () => {
    setLoadState('loading')
    setError('')
    try {
      const nextStatus = await requestStatus()
      setStatus(nextStatus)
      setLoadState('ready')
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
      setLoadState('error')
    }
  }, [requestStatus])

  useEffect(() => {
    if (!isTauri) return

    let active = true
    void requestStatus()
      .then((nextStatus) => {
        if (!active) return
        setStatus(nextStatus)
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
  }, [isTauri, requestStatus])

  const observed = hasObservedRuntimeEvidence(status)
  const statusKey = observed
    ? 'runtimeEvidence.states.observed'
    : status?.collector_state === 'not_configured'
      ? 'runtimeEvidence.states.notConfigured'
      : 'runtimeEvidence.states.unknown'

  return (
    <section className="runtime-evidence-page">
      <header className="runtime-evidence-header">
        <div>
          <h1>{t('runtimeEvidence.title')}</h1>
          <p>{t('runtimeEvidence.subtitle')}</p>
        </div>
        <button
          className="btn btn-secondary"
          type="button"
          onClick={() => void loadStatus()}
          disabled={loadState === 'loading' || !isTauri}
        >
          {loadState === 'loading' ? (
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
            <CircleAlert size={18} />
            <span>{t('runtimeEvidence.unavailable')}</span>
          </div>
        ) : loadState === 'error' ? (
          <div className="runtime-evidence-message error" role="alert">
            <CircleAlert size={18} />
            <div>
              <strong>{t('runtimeEvidence.error')}</strong>
              <span>{error}</span>
            </div>
          </div>
        ) : status ? (
          <>
            <div className={`runtime-evidence-state${observed ? ' observed' : ''}`}>
              <Activity size={20} />
              <div>
                <strong>{t(statusKey)}</strong>
                <span>
                  {observed
                    ? t('runtimeEvidence.observedDetail')
                    : t('runtimeEvidence.unknownDetail')}
                </span>
              </div>
            </div>

            <dl className="runtime-evidence-facts">
              <div>
                <dt>{t('runtimeEvidence.collector')}</dt>
                <dd>{t(`runtimeEvidence.collectors.${status.collector_state}`)}</dd>
              </div>
              <div>
                <dt>{t('runtimeEvidence.contract')}</dt>
                <dd>v{status.schema_version}</dd>
              </div>
              <div>
                <dt>{t('runtimeEvidence.eventChannel')}</dt>
                <dd>{status.event_name}</dd>
              </div>
              <div>
                <dt>{t('runtimeEvidence.lastEvent')}</dt>
                <dd>
                  {status.last_event_at_ms === null
                    ? t('runtimeEvidence.never')
                    : new Date(status.last_event_at_ms).toLocaleString()}
                </dd>
              </div>
            </dl>
          </>
        ) : null}
      </div>
    </section>
  )
}

export default RuntimeEvidencePage
