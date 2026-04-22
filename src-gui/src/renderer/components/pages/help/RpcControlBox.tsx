import { Box, makeStyles } from "@material-ui/core";
import FolderOpenIcon from "@material-ui/icons/FolderOpen";
import PlayArrowIcon from "@material-ui/icons/PlayArrow";
import StopIcon from "@material-ui/icons/Stop";
import PromiseInvokeButton from "renderer/components/PromiseInvokeButton";
import { useIsContextAvailable } from "store/hooks";
import InfoBox from "../../modal/swap/InfoBox";
import CliLogsBox from "../../other/RenderedCliLog";
import { useEffect, useState } from "react";
import { getLogs, startDaemon, isDaemonRunning } from "renderer/rpc";
import { CliLog } from "models/cliModel";

const useStyles = makeStyles((theme) => ({
  actionsOuter: {
    display: "flex",
    gap: theme.spacing(1),
    alignItems: "center",
  },
}));

export default function RpcControlBox() {
  const isContextAvailable = useIsContextAvailable();
  const classes = useStyles();
  const [logs, setLogs] = useState<(CliLog | string)[]>([]);
  const [isDaemonActive, setIsDaemonActive] = useState(false);

  useEffect(() => {
    let interval: NodeJS.Timeout;

    const fetchStatusAndLogs = async () => {
      try {
        const active = await isDaemonRunning();
        setIsDaemonActive(active);

        if (active) {
          const newLogs = await getLogs(null);
          setLogs(newLogs as any);
        }
      } catch (e) {
        // Only log error if context is available to avoid spamming when starting up
        if (isContextAvailable) {
          console.error("Failed to fetch daemon status or logs", e);
        }
      }
    };

    fetchStatusAndLogs();
    interval = setInterval(fetchStatusAndLogs, 2000);

    return () => clearInterval(interval);
  }, [isContextAvailable]);

  return (
    <InfoBox
      title={`Daemon Controller`}
      mainContent={
        isDaemonActive ? (
          <CliLogsBox
            label="Swap Daemon Logs (current session only)"
            logs={logs}
          />
        ) : null
      }
      additionalContent={
        <Box className={classes.actionsOuter}>
          <PromiseInvokeButton
            variant="contained"
            endIcon={<PlayArrowIcon />}
            disabled={!isContextAvailable || isDaemonActive}
            onInvoke={async () => {
              await startDaemon();
              setIsDaemonActive(true);
            }}
          >
            Start Daemon
          </PromiseInvokeButton>
          <PromiseInvokeButton
            variant="contained"
            endIcon={<StopIcon />}
            disabled={!isDaemonActive}
            onInvoke={() => {
              throw new Error("Not implemented");
            }}
          >
            Stop Daemon
          </PromiseInvokeButton>
          <PromiseInvokeButton
            endIcon={<FolderOpenIcon />}
            isIconButton
            size="small"
            tooltipTitle="Open the data directory of the Swap Daemon in your file explorer"
            onInvoke={() => {
              throw new Error("Not implemented");
            }}
          />
        </Box>
      }
      icon={null}
      loading={false}
    />
  );
}
