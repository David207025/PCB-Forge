import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Box,
  Container,
  Typography,
  Grid,
  Button,
  Chip,
  Tooltip,
  IconButton,
  Divider,
  Paper,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  Alert
} from '@mui/material';
import {
  Build as BuildIcon,
  Storage as StorageIcon,
  Refresh as RefreshIcon,
  HelpOutlined as HelpIcon,
  CheckCircle as CheckCircleIcon,
  Cancel as CancelIcon,
  Code as CodeIcon,
  AddBox as AddBoxIcon,
  PictureAsPdf as PdfIcon
} from '@mui/icons-material';

// Standard VS Code Webview API declaration
declare const acquireVsCodeApi: () => { postMessage: (msg: unknown) => void };
const vscodeApi = typeof acquireVsCodeApi === 'function' ? acquireVsCodeApi() : null;

interface TemplateItem {
  name: string;
  isGenerated: boolean;
}

export default function HomePage() {
  const navigate = useNavigate();
  const [templates, setTemplates] = useState<TemplateItem[]>([]);

  useEffect(() => {
    // Requests template status from extension.ts on mount
    if (vscodeApi) {
      vscodeApi.postMessage({ command: 'requestTemplatesState' });
    }

    const handleMessage = (event: MessageEvent) => {
      const msg = event.data;
      if (msg.command === 'setTemplatesState') {
        setTemplates(msg.templates || []);
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  const triggerVSCodeCommand = (commandName: string) => {
    if (vscodeApi) {
      vscodeApi.postMessage({ command: 'runVSCodeCommand', commandName });
    }
  };

  const refreshTemplates = () => {
    if (vscodeApi) {
      vscodeApi.postMessage({ command: 'requestTemplatesState' });
    }
  };

  return (
    <Box sx={{ flexGrow: 1, p: 3, bgcolor: 'background.default', minHeight: '100vh', color: 'text.primary' }}>
      <Container maxWidth="lg">
        {/* Header */}
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 4 }}>
          <Box>
            <Typography variant="h4" gutterBottom>
              PCB Forge Workspace
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Automated KiCad Documentation & Vector Parts Database Engine
            </Typography>
          </Box>
          <Button
            variant="contained"
            color="primary"
            startIcon={<StorageIcon />}
            onClick={() => navigate('/db')}
            size="large"
          >
            Open BOM / Parts DB
          </Button>
        </Box>

        <Grid container spacing={3}>
          {/* Template Status Card */}
          <Grid size={{ xs: 12, md: 7 }}>
            <Paper elevation={2} sx={{ p: 3, borderRadius: 2 }}>
              <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
                <Typography variant="h6">
                  Workspace Templates
                </Typography>
                <Tooltip title="Refresh template status">
                  <IconButton size="small" onClick={refreshTemplates}>
                    <RefreshIcon />
                  </IconButton>
                </Tooltip>
              </Box>
              <Divider sx={{ mb: 2 }} />

              {templates.length === 0 ? (
                <Alert severity="info" sx={{ mt: 1 }}>
                  No template source directories found in <code>~/.pcb-forge/templates/src/</code>.
                </Alert>
              ) : (
                <List>
                  {templates.map((tpl) => (
                    <ListItem key={tpl.name} divider>
                      <ListItemIcon>
                        {tpl.isGenerated ? (
                          <CheckCircleIcon color="success" />
                        ) : (
                          <CancelIcon color="error" />
                        )}
                      </ListItemIcon>
                      <ListItemText
                        primary={tpl.name}
                        secondary={
                          tpl.isGenerated
                            ? 'Schema generated & ready for IDE validation'
                            : 'Schema missing (requires generation)'
                        }
                      />
                      <Chip
                        label={tpl.isGenerated ? 'Generated' : 'Pending'}
                        color={tpl.isGenerated ? 'success' : 'warning'}
                        size="small"
                        variant="outlined"
                      />
                    </ListItem>
                  ))}
                </List>
              )}
            </Paper>
          </Grid>

          {/* Quick Actions Panel */}
          <Grid size={{ xs: 12, md: 5 }}>
            <Paper elevation={2} sx={{ p: 3, borderRadius: 2, mb: 3 }}>
              <Typography variant="h6" gutterBottom>
                Quick Actions
              </Typography>
              <Divider sx={{ mb: 2 }} />
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
                <Button
                  variant="outlined"
                  startIcon={<AddBoxIcon />}
                  onClick={() => triggerVSCodeCommand('pcb-forge.createTemplate')}
                  fullWidth
                >
                  Create New Template Scaffold
                </Button>
                <Button
                  variant="outlined"
                  startIcon={<BuildIcon />}
                  onClick={() => triggerVSCodeCommand('pcb-forge.generateTemplates')}
                  fullWidth
                >
                  Regenerate All Template Schemas
                </Button>
                <Button
                  variant="outlined"
                  startIcon={<PdfIcon />}
                  onClick={() => triggerVSCodeCommand('pcb-forge.generateProject')}
                  fullWidth
                >
                  Compile Project PDF
                </Button>
              </Box>
            </Paper>

            {/* Command Reference Box */}
            <Paper elevation={2} sx={{ p: 3, borderRadius: 2, bgcolor: 'action.hover' }}>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1 }}>
                <HelpIcon color="primary" />
                <Typography variant="h6">
                  Command Reference
                </Typography>
              </Box>
              <Typography variant="body2" color="text.secondary">
                Use the VS Code Command Palette (<code>Ctrl+Shift+P</code> / <code>Cmd+Shift+P</code>) to run background workflows:
              </Typography>
              <List dense>
                <ListItem>
                  <ListItemIcon><CodeIcon fontSize="small" /></ListItemIcon>
                  <ListItemText
                    primary="PCB Forge: Generate BOM"
                    secondary="Parses board CSV and matches vector embeddings in Qdrant."
                  />
                </ListItem>
                <ListItem>
                  <ListItemIcon><CodeIcon fontSize="small" /></ListItemIcon>
                  <ListItemText
                    primary="PCB Forge: Generate Project"
                    secondary="Compiles typst layout and assets into a final PDF file."
                  />
                </ListItem>
                <ListItem>
                  <ListItemIcon><CodeIcon fontSize="small" /></ListItemIcon>
                  <ListItemText
                    primary="PCB Forge: Create Template"
                    secondary="Initializes a raw typst + metadata template directory."
                  />
                </ListItem>
              </List>
            </Paper>
          </Grid>
        </Grid>
      </Container>
    </Box>
  );
}