import { useCallback, useEffect, useState } from "react";
import { RefreshCw, Wifi, WifiOff, Monitor } from "lucide-react";
import { api } from "../api";
import { useStore } from "../store";

interface SyncSettings {
  enabled: boolean;
  deviceId: string;
  deviceName: string;
  port: number | null;
  lastSync: string | null;
}

interface SyncPeer {
  deviceId: string;
  deviceName: string;
  host: string;
  port: number;
  lastSeen: string;
}

export function SyncSection() {
  const showToast = useStore((s) => s.showToast);
  const [settings, setSettings] = useState<SyncSettings | null>(null);
  const [peers, setPeers] = useState<SyncPeer[]>([]);
  const [syncing, setSyncing] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [s, p] = await Promise.all([
        api.getSyncSettings(),
        api.getSyncPeers(),
      ]);
      setSettings(s);
      setPeers(p);
    } catch {
      /* settings table might not exist yet */
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  const toggleSync = async () => {
    if (!settings) return;
    try {
      await api.setSyncEnabled(!settings.enabled);
      showToast(
        settings.enabled
          ? "LAN sync disabled"
          : "LAN sync enabled — restart Keepr for discovery to begin",
      );
      await refresh();
    } catch (e) {
      showToast("Could not toggle sync: " + String(e));
    }
  };

  const doSync = async () => {
    setSyncing(true);
    try {
      const results = await api.syncNow();
      if (results.length === 0) {
        showToast("No sync results");
      } else {
        const total = results.reduce(
          (acc, r) => ({
            pulled: acc.pulled + r.notesPulled,
            pushed: acc.pushed + r.notesPushed,
          }),
          { pulled: 0, pushed: 0 },
        );
        showToast(
          `Synced with ${results.length} peer(s): ${total.pulled} pulled, ${total.pushed} pushed`,
        );
        useStore.getState().load();
      }
      await refresh();
    } catch (e) {
      showToast("Sync failed: " + String(e));
    } finally {
      setSyncing(false);
    }
  };

  if (!settings) return null;

  const btnClass =
    "w-full sm:w-auto justify-start whitespace-normal text-left flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50";

  return (
    <div>
      <div className="font-medium flex items-center gap-2">
        {settings.enabled ? (
          <Wifi size={16} className="text-green-500" aria-hidden />
        ) : (
          <WifiOff size={16} className="opacity-40" aria-hidden />
        )}
        LAN Sync
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
        Sync notes between Keepr instances on the same local network.
        No cloud server, no account — devices discover each other via
        mDNS and exchange notes directly. Single-user only.
      </p>

      <div className="flex gap-2 mt-3 flex-wrap">
        <button onClick={toggleSync} className={btnClass}>
          {settings.enabled ? (
            <>
              <WifiOff size={14} aria-hidden /> Disable sync
            </>
          ) : (
            <>
              <Wifi size={14} aria-hidden /> Enable sync
            </>
          )}
        </button>
        {settings.enabled && (
          <button
            onClick={doSync}
            disabled={syncing || peers.length === 0}
            className={btnClass}
          >
            <RefreshCw
              size={14}
              className={syncing ? "animate-spin" : ""}
              aria-hidden
            />
            {syncing ? "Syncing…" : "Sync now"}
          </button>
        )}
      </div>

      {settings.enabled && (
        <div className="mt-3 space-y-2">
          <div className="text-xs opacity-60">
            Device: {settings.deviceName} ({settings.deviceId.slice(0, 8)}…)
            {settings.port && ` · Port ${settings.port}`}
          </div>

          {settings.lastSync && (
            <div className="text-xs opacity-60">
              Last sync: {new Date(settings.lastSync).toLocaleString()}
            </div>
          )}

          <div className="text-xs font-medium mt-2">
            Discovered peers ({peers.length})
          </div>
          {peers.length === 0 ? (
            <div className="text-xs opacity-50">
              No other Keepr instances found on this network
            </div>
          ) : (
            <div className="space-y-1">
              {peers.map((p) => (
                <div
                  key={p.deviceId}
                  className="flex items-center gap-2 text-xs px-2 py-1.5 rounded bg-black/5 dark:bg-white/5"
                >
                  <Monitor size={12} aria-hidden />
                  <span className="font-medium">{p.deviceName}</span>
                  <span className="opacity-50">
                    {p.host}:{p.port}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
