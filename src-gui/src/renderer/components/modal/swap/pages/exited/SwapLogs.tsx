import { Box, Typography, makeStyles } from "@material-ui/core";
import { useEffect, useState } from "react";
import { getLogs } from "renderer/rpc";

const useStyles = makeStyles((theme) => ({
  logContainer: {
    backgroundColor: theme.palette.background.default,
    padding: theme.spacing(1),
    borderRadius: theme.shape.borderRadius,
    marginTop: theme.spacing(2),
    height: "250px",
    overflowY: "auto",
    fontFamily: "monospace",
    fontSize: "0.75rem",
    border: `1px solid ${theme.palette.divider}`,
    color: theme.palette.text.secondary,
  },
  logLine: {
    whiteSpace: "pre-wrap",
    wordBreak: "break-all",
    marginBottom: "4px",
  },
}));

export default function SwapLogs({ swapId }: { swapId: string }) {
  const classes = useStyles();
  const [logs, setLogs] = useState<string[]>([]);

  useEffect(() => {
    getLogs(swapId).then(setLogs).catch(console.error);
  }, [swapId]);

  return (
    <Box>
      <Typography variant="subtitle2">Swap Logs</Typography>
      <Box className={classes.logContainer}>
        {logs.length === 0 ? (
          <Typography variant="caption">Loading logs or no logs available...</Typography>
        ) : (
          logs.map((log, index) => (
            <div key={index} className={classes.logLine}>
              {log}
            </div>
          ))
        )}
      </Box>
    </Box>
  );
}
