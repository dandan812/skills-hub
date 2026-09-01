import { describe, expect, it } from 'vitest'
import { filterRuntimeRows, findAgent, findSession, sessionKey } from './status'
import type { RuntimeEvidenceOverview, RuntimeSkillRow } from './types'

const row = (skillId: string, skillName = skillId): RuntimeSkillRow => ({
  skillId,
  skillName,
  installed: 'yes',
  assigned: 'no',
  loaded: 'unknown',
  lastCall: { state: 'unknown' },
  source: 'managementCatalog',
  reason: 'sessionEvidenceMissing',
})

const overview: RuntimeEvidenceOverview = {
  schemaVersion: 1,
  status: 'ready',
  import: { accepted: 0, duplicate: 0, rejected: 0, reasons: [] },
  inboxPath: 'runtime-hooks/skill-runtime-v1.jsonl',
  eventCount: 0,
  agents: [{
    agentId: 'codex',
    catalogState: 'available',
    sessions: [{
      sessionId: null,
      state: 'unknown',
      startedAt: null,
      lastObservedAt: null,
      skills: [row('code'), row('paper-digest', 'Paper Digest')],
    }],
  }],
}

describe('runtime evidence selectors', () => {
  it('selects a fixed agent and falls back to its first session', () => {
    const agent = findAgent(overview, 'codex')
    const session = findSession(agent, 'missing')

    expect(agent?.agentId).toBe('codex')
    expect(session?.state).toBe('unknown')
    expect(sessionKey(session!)).toBe('__unknown__')
  })

  it('filters by display name or runtime Skill ID', () => {
    const rows = overview.agents[0].sessions[0].skills

    expect(filterRuntimeRows(rows, 'paper')).toEqual([rows[1]])
    expect(filterRuntimeRows(rows, 'CODE')).toEqual([rows[0]])
    expect(filterRuntimeRows(rows, '  ')).toEqual(rows)
  })
})
