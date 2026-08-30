export default function HomePage()  {
  const isCliMissing = (window as any).IS_CLI_AVAILABLE === false;

  return (
    <div>
      {isCliMissing && (
        <div style={{ backgroundColor: 'var(--vscode-inputValidation-errorBackground)', border: '1px solid var(--vscode-inputValidation-errorBorder)', padding: '10px', marginBottom: '15px', borderRadius: '4px' }}>
          ⚠️ <strong>Warning:</strong> The companion CLI tool was not detected on your system path. Some background features may be limited.
        </div>
      )}
      <h1>PCB Forge</h1>
    </div>
  )
}