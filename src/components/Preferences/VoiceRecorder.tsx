import { useEffect, useRef, useState } from "react";
import { Glyph, Tag, Zone } from "@overtone-foundry/app-kit";

/** Record a short reference clip with the Mac's microphone and hand back a 16-bit mono WAV.
 *  Capture only: the Hub owns all playback, so there is no preview here. */
export default function VoiceRecorder({ onDone, onCancel }: { onDone: (wav: Uint8Array, seconds: number) => void; onCancel: () => void }) {
  const [state, setState] = useState<"idle" | "recording">("idle");
  const [seconds, setSeconds] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const session = useRef<{ ctx: AudioContext; stream: MediaStream; node: ScriptProcessorNode; chunks: Float32Array[]; timer: number; started: number } | null>(null);

  const stop = () => {
    const s = session.current;
    if (!s) return;
    session.current = null;
    clearInterval(s.timer);
    s.node.disconnect();
    s.stream.getTracks().forEach((t) => t.stop());
    const rate = s.ctx.sampleRate;
    s.ctx.close().catch(() => {});
    const total = s.chunks.reduce((n, c) => n + c.length, 0);
    const pcm = new Float32Array(total);
    let off = 0;
    for (const c of s.chunks) { pcm.set(c, off); off += c.length; }
    setState("idle");
    onDone(encodeWav(pcm, rate), total / rate);
  };

  const start = async () => {
    setErr(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
      const ctx = new AudioContext();
      const source = ctx.createMediaStreamSource(stream);
      const node = ctx.createScriptProcessor(4096, 1, 1);
      const chunks: Float32Array[] = [];
      node.onaudioprocess = (e) => chunks.push(new Float32Array(e.inputBuffer.getChannelData(0)));
      // WebKit only runs the processor when it reaches the destination; a muted gain keeps the
      // microphone out of the speakers.
      const mute = ctx.createGain();
      mute.gain.value = 0;
      source.connect(node);
      node.connect(mute);
      mute.connect(ctx.destination);
      const started = Date.now();
      const timer = window.setInterval(() => {
        const secs = (Date.now() - started) / 1000;
        setSeconds(secs);
        if (secs >= MAX_SECONDS) stop();
      }, 100);
      session.current = { ctx, stream, node, chunks, timer, started };
      setSeconds(0);
      setState("recording");
    } catch (e) {
      const name = (e as { name?: string })?.name;
      setErr(name === "NotAllowedError"
        ? "Microphone access was denied. Allow Speech Hub in System Settings → Privacy & Security → Microphone, then try again."
        : `Couldn't start recording: ${(e as Error)?.message ?? String(e)}`);
    }
  };

  useEffect(() => () => { const s = session.current; if (s) { clearInterval(s.timer); s.stream.getTracks().forEach((t) => t.stop()); s.ctx.close().catch(() => {}); } }, []);

  const cancel = () => {
    const s = session.current;
    if (s) { session.current = null; clearInterval(s.timer); s.node.disconnect(); s.stream.getTracks().forEach((t) => t.stop()); s.ctx.close().catch(() => {}); }
    onCancel();
  };

  return (
    <Zone className="editing" title="Record a voice" aside={state === "recording" ? <Tag tone="live">recording · {seconds.toFixed(1)} s</Tag> : <Tag>up to {MAX_SECONDS} s</Tag>}>
      <p>Speak naturally for 10–20 seconds in a quiet room. Something like:</p>
      <blockquote className="paper" style={{ margin: "0 0 12px" }}>
        “The autumn leaves drifted across the quiet street while the kettle came to a boil. I set two cups on the table, opened the window a little, and listened to the rain begin.”
      </blockquote>
      <div className="row">
        {state === "idle"
          ? <button className="primary" onClick={start}><Glyph name="audio" /> Start recording</button>
          : <button className="primary stop" onClick={stop}><Glyph name="stop" /> Stop</button>}
        <button className="quiet" onClick={cancel}>Cancel</button>
      </div>
      {err && <pre className="error">{err}</pre>}
    </Zone>
  );
}

const MAX_SECONDS = 30;

/** 16-bit PCM mono WAV. */
export function encodeWav(samples: Float32Array, sampleRate: number): Uint8Array {
  const bytes = 44 + samples.length * 2;
  const buf = new ArrayBuffer(bytes);
  const v = new DataView(buf);
  const str = (o: number, s: string) => { for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i)); };
  str(0, "RIFF"); v.setUint32(4, bytes - 8, true); str(8, "WAVE");
  str(12, "fmt "); v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, sampleRate, true); v.setUint32(28, sampleRate * 2, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true);
  str(36, "data"); v.setUint32(40, samples.length * 2, true);
  let o = 44;
  for (let i = 0; i < samples.length; i++, o += 2) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    v.setInt16(o, s < 0 ? s * 0x8000 : s * 0x7fff, true);
  }
  return new Uint8Array(buf);
}
