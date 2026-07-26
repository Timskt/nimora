import { useCallback, useEffect, useRef, useState } from "react";
import type { NimoraEvent } from "@nimora/schemas";
import { desktopApi } from "../platform/desktop";
import {
  describeDrainNotice,
  eventSourceCategory,
  eventSourceLabel,
  formatEventTimestamp,
  mergeDrainedEvents,
  shortTraceId,
  tallyEventSources,
  RUNTIME_EVENT_BUFFER_CAP,
} from "./runtimeEventDiagnostics";

export function RuntimeEventDiagnosticsPanel({ disabled }: { disabled: boolean }) {
  const [events, setEvents] = useState<NimoraEvent[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const autoTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const [autoDrain, setAutoDrain] = useState(false);

  const drainOnce = useCallback(async (announce: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const drained = await desktopApi.drainEvents();
      let freshCount = 0;
      setEvents((buffer) => {
        const merged = mergeDrainedEvents(buffer, drained);
        freshCount = merged.length - buffer.length;
        return merged;
      });
      if (announce) setNotice(describeDrainNotice(drained.length, Math.max(0, freshCount)));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "排空运行时事件失败");
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (!autoDrain) {
      if (autoTimer.current) { clearInterval(autoTimer.current); autoTimer.current = null; }
      return;
    }
    autoTimer.current = setInterval(() => { void drainOnce(false); }, 4_000);
    return () => { if (autoTimer.current) { clearInterval(autoTimer.current); autoTimer.current = null; } };
  }, [autoDrain, drainOnce]);

  const tallies = tallyEventSources(events);

  return (
    <section className="skill-lifecycle-panel runtime-event-diagnostics" aria-label="运行时事件诊断流">
      <header className="skill-lifecycle-head">
        <div>
          <small>DEVELOPER DIAGNOSTICS</small>
          <h3>运行时事件诊断流</h3>
        </div>
        <div className="skill-head-actions">
          <label className="runtime-event-auto">
            <input type="checkbox" checked={autoDrain} disabled={disabled} onChange={(event) => setAutoDrain(event.target.checked)} />
            <span>自动排空</span>
          </label>
          <button className="skill-refresh" disabled={disabled || busy} onClick={() => void drainOnce(true)} type="button">排空事件</button>
        </div>
      </header>

      <p className="runtime-event-privacy">
        面向开发者/极客的原始事件流：只展示事件类型、来源、TraceId 与时间戳等元数据，绝不显示事件正文（`data`）、桌面内容或密钥。排空是破坏性的——事件从运行时队列取出后累计到此处，最多保留 {RUNTIME_EVENT_BUFFER_CAP} 条。
      </p>

      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}

      {tallies.length > 0 ? (
        <div className="runtime-event-tally">
          {tallies.map((tally) => (
            <span className="runtime-event-tally-chip" data-source={tally.category} key={tally.category}>
              {tally.label} · {tally.count}
            </span>
          ))}
        </div>
      ) : null}

      {events.length === 0 ? (
        <div className="skill-empty-state">
          <span>✦</span>
          <p>诊断流为空。点击「排空事件」把运行时队列中的原始事件取出到此处检查。浏览器预览不会伪造事件，此流只在原生桌面运行时有真实数据。</p>
        </div>
      ) : (
        <ul className="runtime-event-list">
          {events.map((event) => {
            const category = eventSourceCategory(event.source);
            return (
              <li className="runtime-event-row" data-source={category} key={event.id}>
                <div className="runtime-event-row-head">
                  <code>{event.eventType}</code>
                  <span className="runtime-event-source" data-source={category}>{eventSourceLabel(category)}</span>
                </div>
                <p className="runtime-event-meta">
                  {event.source} · Trace {shortTraceId(event.traceId)} · {formatEventTimestamp(event.timestamp)}
                </p>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
