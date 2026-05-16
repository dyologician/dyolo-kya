function AiToolsHub() {
  const [sub, setSub] = React.useState('assistant');
  return h('div', null,
    h('div', { style: { display: 'flex', gap: 8, marginBottom: 16 } },
      h('button', {
        className: 'btn ' + (sub === 'assistant' ? 'btn-p' : 'btn-s') + ' btn-sm',
        onClick: () => setSub('assistant')
      }, '🧠 AI Assistant'),
      h('button', {
        className: 'btn ' + (sub === 'localllm' ? 'btn-p' : 'btn-s') + ' btn-sm',
        onClick: () => setSub('localllm')
      }, '🤖 Local AI Setup')
    ),
    sub === 'assistant' && h(AiAssistant, null),
    sub === 'localllm'  && h(LocalLlmConnect, null)
  );
}
