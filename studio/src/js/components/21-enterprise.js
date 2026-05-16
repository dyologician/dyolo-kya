// ─────────────────────────────────────────────────────────────────────────────
// ENTERPRISE PANEL — JWT Exchange · Webhook · Tenant
// ─────────────────────────────────────────────────────────────────────────────

function EnterprisePanel() {
  const [sub, setSub] = React.useState('jwt');

  const ETABS = [
    { id: 'jwt',     label: 'JWT Exchange', icon: '🔑' },
    { id: 'webhook', label: 'Webhook',      icon: '📡' },
    { id: 'tenant',  label: 'Tenant',       icon: '🏢' },
  ];

  const CONTENT = {
    jwt:     h(JwtExchangePanel, null),
    webhook: h(WebhookPanel,     null),
    tenant:  h(TenantPanel,      null),
  };

  return h('div', null,
    h('div', { style: { display: 'flex', gap: 6, marginBottom: 16, flexWrap: 'wrap' } },
      ETABS.map(t =>
        h('button', {
          key: t.id,
          className: 'btn btn-sm ' + (sub === t.id ? 'btn-p' : 'btn-s'),
          onClick: () => setSub(t.id),
        }, t.icon + '\u2009' + t.label)
      )
    ),
    CONTENT[sub]
  );
}

// ── JWT Exchange ──────────────────────────────────────────────────────────────

function JwtExchangePanel() {
  const { settings, api } = useContext(Ctx);

  const [token,      setToken]      = React.useState('');
  const [delegatePk, setDelegatePk] = React.useState('');
  const [caps,       setCaps]       = React.useState('read.data,write.data');
  const [ttl,        setTtl]        = React.useState('3600');
  const [result,     setResult]     = React.useState(null);
  const [err,        setErr]        = React.useState('');
  const [busy,       setBusy]       = React.useState(false);

  async function exchange() {
    setErr(''); setResult(null); setBusy(true);
    const capabilities = caps.split(',').map(s => s.trim()).filter(Boolean);
    const { ok, data } = await api('POST', '/v1/jwt/exchange', {
      token,
      delegate_pk_hex: delegatePk,
      capabilities,
      ttl_seconds: parseInt(ttl) || 3600,
    });
    setBusy(false);
    if (ok) setResult(data);
    else setErr(data && data.error ? data.error : 'Exchange failed — verify A1_JWT_JWKS_URL is set on the gateway');
  }

  return h('div', { className: 'card' },
    h('div', { className: 'ctitle' }, 'JWT → Delegation Cert'),
    h('p', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)', marginBottom: 12 } },
      'Exchange an OIDC/OAuth2 JWT bearer token for a scoped A1 DelegationCert. ' +
      'Set A1_JWT_JWKS_URL on the gateway to enable this endpoint.'
    ),
    h('div', { className: 'field' },
      h('label', { className: 'lbl' }, 'JWT Bearer Token'),
      h('textarea', {
        className: 'inp inp-mono', rows: 4,
        placeholder: 'eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9…',
        value: token, onChange: e => setToken(e.target.value),
      })
    ),
    h('div', { className: 'field' },
      h('label', { className: 'lbl' }, 'Agent Public Key (hex)'),
      h('input', {
        className: 'inp inp-mono',
        placeholder: '64-char Ed25519 public key hex',
        value: delegatePk, onChange: e => setDelegatePk(e.target.value),
      })
    ),
    h('div', { className: 'g2' },
      h('div', { className: 'field' },
        h('label', { className: 'lbl' }, 'Capabilities (comma-separated)'),
        h('input', { className: 'inp inp-mono', value: caps, onChange: e => setCaps(e.target.value) })
      ),
      h('div', { className: 'field' },
        h('label', { className: 'lbl' }, 'TTL (seconds)'),
        h('input', { className: 'inp inp-mono', type: 'number', min: 60, value: ttl, onChange: e => setTtl(e.target.value) })
      )
    ),
    h('button', {
      className: 'btn btn-p',
      onClick: exchange,
      disabled: busy || !token || !delegatePk,
    }, busy ? 'Exchanging…' : 'Exchange JWT'),
    err && h(Alert, { msg: err, type: 'error' }),
    result && h('div', { className: 'card', style: { background: 'var(--bg2)', marginTop: 12 } },
      h('div', { className: 'ctitle' }, 'Issued Cert'),
      h('div', { className: 'g2', style: { marginBottom: 8 } },
        h('div', null, h('div', { className: 'lbl' }, 'Subject'), h('div', { className: 'code-val' }, result.jwt_subject || '—')),
        h('div', null, h('div', { className: 'lbl' }, 'Issuer'),  h('div', { className: 'code-val' }, result.jwt_issuer  || '—'))
      ),
      h('div', { className: 'field' },
        h('div', { className: 'lbl' }, 'Fingerprint'),
        h(TruncId, { val: result.fingerprint_hex })
      ),
      h('div', { className: 'field' },
        h('div', { className: 'lbl' }, 'Capabilities'),
        h('div', { className: 'code-val' }, (result.capabilities || []).join(', '))
      ),
      h('div', { className: 'field' },
        h('div', { className: 'lbl' }, 'Expires'),
        h('div', { className: 'code-val' },
          result.expires_at_unix ? new Date(result.expires_at_unix * 1000).toLocaleString() : '—'
        )
      )
    )
  );
}

// ── Webhook ───────────────────────────────────────────────────────────────────

function WebhookPanel() {
  const { api } = useContext(Ctx);

  const [status,  setStatus]  = React.useState(null);
  const [testMsg, setTestMsg] = React.useState('');
  const [busy,    setBusy]    = React.useState(false);

  React.useEffect(() => {
    api('GET', '/v1/webhook/status').then(({ ok, data }) => {
      if (ok) setStatus(data);
    });
  }, []);

  async function sendTest() {
    setBusy(true); setTestMsg('');
    const { ok, data } = await api('POST', '/v1/webhook/test', {});
    setBusy(false);
    setTestMsg(ok ? (data.message || 'Test event dispatched') : (data.error || 'Failed'));
  }

  return h('div', null,
    h('div', { className: 'card', style: { marginBottom: 12 } },
      h('div', { className: 'ctitle' }, 'Webhook Status'),
      status
        ? h('div', null,
            h('div', { className: 'g2', style: { marginBottom: 10 } },
              h('div', null,
                h('div', { className: 'lbl' }, 'Endpoint'),
                h('span', { className: 'badge ' + (status.enabled ? 'badge-ok' : 'badge-dim') },
                  status.enabled ? '✓ Active' : 'Not configured')
              ),
              h('div', null,
                h('div', { className: 'lbl' }, 'Signature'),
                h('span', { className: 'badge ' + (status.signed ? 'badge-ok' : 'badge-dim') },
                  status.signed ? '✓ HMAC-BLAKE3' : 'Unsigned')
              )
            ),
            status.url && h('div', { className: 'field' },
              h('div', { className: 'lbl' }, 'Receiving at'),
              h('div', { className: 'code-val' }, status.url)
            ),
            h('button', {
              className: 'btn btn-p btn-sm',
              style: { marginTop: 10 },
              onClick: sendTest,
              disabled: busy || !status.enabled,
              title: !status.enabled ? 'Set A1_WEBHOOK_URL on gateway to enable' : '',
            }, busy ? 'Sending…' : 'Send Test Event'),
            testMsg && h('div', { style: { marginTop: 8, fontSize: 'var(--fsm)', color: 'var(--t2)' } }, testMsg)
          )
        : h('div', { className: 'empty' }, 'Fetching webhook status…')
    ),
    h('div', { className: 'card' },
      h('div', { className: 'ctitle' }, 'Configuration'),
      h('p', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)', marginBottom: 8 } },
        'Set these environment variables on the gateway to enable real-time SIEM event push:'
      ),
      h('pre', { style: { fontSize: 'var(--fxs)', background: 'var(--bg2)', padding: '10px 12px', borderRadius: 6, overflowX: 'auto' } },
        'A1_WEBHOOK_URL=https://ingest.example.com/a1-events\nA1_WEBHOOK_SECRET=your-32-byte-secret'
      ),
      h('p', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)', marginTop: 8, lineHeight: '1.7' } },
        'Every authorization result fires a signed POST. ' +
        'Verify with the X-A1-Webhook-Signature header using BLAKE3-HMAC.'
      )
    )
  );
}

// ── Tenant ────────────────────────────────────────────────────────────────────

function TenantPanel() {
  const { settings } = useContext(Ctx);
  const gwUrl = (settings && settings.gwUrl) ? settings.gwUrl.replace(/\/$/, '') : 'http://localhost:8080';

  const [tenantId, setTenantId] = React.useState('');
  const [info,     setInfo]     = React.useState(null);
  const [config,   setConfig]   = React.useState(null);
  const [err,      setErr]      = React.useState('');
  const [busy,     setBusy]     = React.useState(false);

  async function fetchTenant() {
    setErr(''); setInfo(null); setConfig(null); setBusy(true);
    const hdrs = { 'Content-Type': 'application/json' };
    if (tenantId) hdrs['X-A1-Tenant-ID'] = tenantId;

    async function getJson(path) {
      try {
        const r = await fetch(gwUrl + path, { headers: hdrs });
        return r.ok ? r.json() : null;
      } catch { return null; }
    }

    const [infoData, cfgData] = await Promise.all([
      getJson('/v1/tenant/info'),
      getJson('/v1/tenant/config'),
    ]);
    setBusy(false);

    if (!infoData && !cfgData) {
      setErr('Could not reach gateway. Check Settings → Gateway URL.');
    } else {
      if (infoData) setInfo(infoData);
      if (cfgData)  setConfig(cfgData);
    }
  }

  React.useEffect(() => { fetchTenant(); }, []);

  return h('div', null,
    h('div', { className: 'card', style: { marginBottom: 12 } },
      h('div', { className: 'ctitle' }, 'Tenant Context'),
      h('div', { style: { display: 'flex', gap: 8, marginBottom: 10, alignItems: 'flex-end' } },
        h('div', { className: 'field', style: { flex: 1, marginBottom: 0 } },
          h('label', { className: 'lbl' }, 'Tenant ID (X-A1-Tenant-ID)'),
          h('input', {
            className: 'inp inp-mono',
            placeholder: 'acme-corp (blank = default tenant)',
            value: tenantId,
            onChange: e => setTenantId(e.target.value),
          })
        ),
        h('button', { className: 'btn btn-p btn-sm', onClick: fetchTenant, disabled: busy }, busy ? '…' : 'Fetch')
      ),
      err && h(Alert, { msg: err, type: 'error' }),
      info && h('div', { className: 'g2', style: { marginTop: 8 } },
        h('div', null,
          h('div', { className: 'lbl' }, 'Multi-Tenant'),
          h('span', { className: 'badge ' + (info.multi_tenant_enabled ? 'badge-ok' : 'badge-dim') },
            info.multi_tenant_enabled ? 'Enabled' : 'Disabled')
        ),
        h('div', null,
          h('div', { className: 'lbl' }, 'Required'),
          h('span', { className: 'badge ' + (info.tenant_required ? 'badge-warn' : 'badge-dim') },
            info.tenant_required ? 'Required' : 'Optional')
        ),
        info.active_tenant && h('div', null,
          h('div', { className: 'lbl' }, 'Active Tenant'),
          h('div', { className: 'code-val' }, info.active_tenant)
        ),
        info.allowlist && info.allowlist.length > 0 && h('div', null,
          h('div', { className: 'lbl' }, 'Allowlist'),
          h('div', { className: 'code-val' }, info.allowlist.join(', '))
        ),
        info.store_prefix && h('div', { style: { gridColumn: '1 / -1' } },
          h('div', { className: 'lbl' }, 'Store Key Prefix'),
          h('div', { className: 'code-val', style: { fontSize: 'var(--fxs)' } }, info.store_prefix)
        )
      )
    ),
    config && h('div', { className: 'card' },
      h('div', { className: 'ctitle' }, 'Capability Limits'),
      h('div', { className: 'g2', style: { marginBottom: 8 } },
        h('div', null,
          h('div', { className: 'lbl' }, 'Max Chain Depth'),
          h('div', { className: 'code-val' }, config.max_chain_depth)
        ),
        h('div', null,
          h('div', { className: 'lbl' }, 'Max TTL'),
          h('div', { className: 'code-val' }, config.max_ttl_seconds + 's')
        )
      ),
      config.allowed_caps && config.allowed_caps.length > 0
        ? h('div', { className: 'field' },
            h('div', { className: 'lbl' }, 'Allowed Capabilities'),
            h('div', { style: { display: 'flex', gap: 4, flexWrap: 'wrap', marginTop: 4 } },
              config.allowed_caps.map(c => h('span', { key: c, className: 'badge badge-dim' }, c))
            )
          )
        : h('div', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)' } }, 'All capabilities permitted for this tenant.'),
      h('div', { style: { marginTop: 12 } },
        h('p', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)', marginBottom: 6 } }, 'Configure on gateway:'),
        h('pre', { style: { fontSize: 'var(--fxs)', background: 'var(--bg2)', padding: '10px 12px', borderRadius: 6 } },
          'A1_MULTI_TENANT=true\nA1_TENANT_REQUIRED=true\nA1_TENANT_ALLOWLIST=acme,globex\nA1_TENANT_ACME_CAPS=trade.equity,portfolio.read'
        )
      )
    )
  );
}
