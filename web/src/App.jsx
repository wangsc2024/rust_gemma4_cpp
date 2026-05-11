import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';

const sampleDigest = {
  title: 'AI 每日新聞洞察',
  generated_at: '尚未產生',
  ntfy_topic: 'https://ntfy.sh/wangsc_ainews',
  sent: false,
  items: [],
  markdown: '# 尚未產生新聞洞察\n\n請先執行 `cargo run -- news --dry-run` 或正式排程。',
};

function App() {
  const [digest, setDigest] = useState(sampleDigest);
  const [error, setError] = useState('');
  const [rawJson, setRawJson] = useState('');

  useEffect(() => {
    fetch('/latest-news.example.json')
      .then((response) => (response.ok ? response.json() : sampleDigest))
      .then((data) => {
        setDigest(data);
        setRawJson(JSON.stringify(data, null, 2));
      })
      .catch(() => {
        setDigest(sampleDigest);
        setRawJson(JSON.stringify(sampleDigest, null, 2));
      });
  }, []);

  const stats = useMemo(
    () => [
      { label: '新聞數', value: digest.items?.length ?? 0 },
      { label: 'ntfy topic', value: digest.ntfy_topic },
      { label: '發送狀態', value: digest.sent ? '已發送' : '未發送 / dry-run' },
    ],
    [digest],
  );

  function applyJson() {
    try {
      const parsed = JSON.parse(rawJson);
      setDigest(parsed);
      setError('');
    } catch (err) {
      setError(`JSON 格式錯誤：${err.message}`);
    }
  }

  return (
    <main className="shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Rust + llama-cli + Gemma 4 E4B Q4_K_M + React</p>
          <h1>{digest.title}</h1>
          <p className="subtitle">
            每日收集 AI RSS 新聞，交給本機 Gemma GGUF 生成繁體中文洞察，並可推送至 ntfy.sh/wangsc_ainews。
          </p>
        </div>
        <div className="command-card">
          <span>每日排程命令</span>
          <code>./scripts/run_daily_news.sh</code>
        </div>
      </section>

      <section className="stats">
        {stats.map((stat) => (
          <article key={stat.label}>
            <span>{stat.label}</span>
            <strong>{stat.value}</strong>
          </article>
        ))}
      </section>

      <section className="grid">
        <article className="panel digest-panel">
          <div className="panel-heading">
            <h2>Gemma 產出洞察</h2>
            <time>{digest.generated_at}</time>
          </div>
          <pre className="markdown-preview">{digest.markdown}</pre>
        </article>

        <article className="panel">
          <div className="panel-heading">
            <h2>新聞來源</h2>
            <span>{digest.items?.length ?? 0} 則</span>
          </div>
          <div className="news-list">
            {(digest.items ?? []).map((item) => (
              <a className="news-item" key={`${item.source}-${item.title}`} href={item.url} target="_blank" rel="noreferrer">
                <span>{item.source}</span>
                <strong>{item.title}</strong>
                <small>{item.published ?? 'unknown date'}</small>
                <p>{item.summary}</p>
              </a>
            ))}
          </div>
        </article>
      </section>

      <section className="panel json-panel">
        <div className="panel-heading">
          <h2>貼上 latest-news JSON 預覽</h2>
          <button type="button" onClick={applyJson}>更新預覽</button>
        </div>
        {error && <p className="error">{error}</p>}
        <textarea value={rawJson} onChange={(event) => setRawJson(event.target.value)} />
      </section>
    </main>
  );
}

createRoot(document.getElementById('root')).render(<App />);
