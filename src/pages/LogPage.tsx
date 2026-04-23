import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ChevronRight } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Card, CardContent } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { getUsageLogs, listChannels } from "@/lib/api";
import type { UsageLogFilter } from "@/types";

export function LogPage() {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<UsageLogFilter>({ page: 1, page_size: 100 });
  const [hoveredOtherId, setHoveredOtherId] = useState<number | null>(null);

  const { data: result, isLoading } = useQuery({
    queryKey: ["usageLogs", filter],
    queryFn: () => getUsageLogs(filter),
  });

  const { data: channels } = useQuery({
    queryKey: ["channels"],
    queryFn: listChannels,
  });

  const setTimeRange = (range: string) => {
    const now = Date.now() / 1000;
    let start: number;
    switch (range) {
      case "today":
        start = now - 86400;
        break;
      case "7d":
        start = now - 7 * 86400;
        break;
      case "30d":
        start = now - 30 * 86400;
        break;
      default:
        start = 0;
    }
    setFilter((f) => ({ ...f, start_time: start || undefined, end_time: undefined }));
  };

  const logs = result?.items || [];
  const totalPrompt = logs.reduce((sum, log) => sum + log.prompt_tokens, 0);
  const totalCompletion = logs.reduce((sum, log) => sum + log.completion_tokens, 0);
  const successCount = logs.filter((log) => log.success).length;

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("log.title")}</h1>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3 mb-4 flex-wrap">
        <Input
          className="w-52 h-8"
          placeholder={t("log.filter.model")}
          value={filter.model ?? ""}
          onChange={(e) =>
            setFilter((f) => ({
              ...f,
              model: e.target.value || undefined,
              page: 1,
            }))
          }
        />

        <Input
          className="w-52 h-8"
          placeholder="Request ID"
          value={filter.request_id ?? ""}
          onChange={(e) =>
            setFilter((f) => ({
              ...f,
              request_id: e.target.value || undefined,
              page: 1,
            }))
          }
        />

        <Select onValueChange={setTimeRange}>
          <SelectTrigger className="w-32 h-8">
            <SelectValue placeholder={t("log.filter.timeRange")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="today">{t("log.filter.today")}</SelectItem>
            <SelectItem value="7d">{t("log.filter.sevenDays")}</SelectItem>
            <SelectItem value="30d">{t("log.filter.thirtyDays")}</SelectItem>
            <SelectItem value="all">{t("log.filter.all")}</SelectItem>
          </SelectContent>
        </Select>

        <Select
          onValueChange={(v) =>
            setFilter((f) => ({ ...f, channel_id: v === "all" ? undefined : v }))
          }
        >
          <SelectTrigger className="w-36 h-8">
            <SelectValue placeholder={t("log.filter.channel")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t("log.filter.all")}</SelectItem>
            {channels?.map((c) => (
              <SelectItem key={c.id} value={c.id}>
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          onValueChange={(v) =>
            setFilter((f) => ({
              ...f,
              success: v === "all" ? undefined : v === "success",
            }))
          }
        >
          <SelectTrigger className="w-28 h-8">
            <SelectValue placeholder={t("log.filter.status")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t("log.filter.all")}</SelectItem>
            <SelectItem value="success">{t("log.success")}</SelectItem>
            <SelectItem value="failed">{t("log.failed")}</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="grid gap-4 md:grid-cols-4 mb-4">
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.recentLogs")}</div>
            <div className="text-2xl font-semibold mt-1">{logs.length}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.promptTokens")}</div>
            <div className="text-2xl font-semibold mt-1">{totalPrompt}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.completionTokens")}</div>
            <div className="text-2xl font-semibold mt-1">{totalCompletion}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.successRate")}</div>
            <div className="text-2xl font-semibold mt-1">
              {logs.length ? `${((successCount / logs.length) * 100).toFixed(1)}%` : "0%"}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Table */}
      <div className="rounded-md border overflow-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b bg-muted/50">
              <th className="px-3 py-2 text-left font-medium">{t("log.time")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.channel")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.token")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.model")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.duration")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.promptTokens")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.completionTokens")}</th>
              <th className="px-3 py-2 text-left font-medium">{t("log.details")}</th>
            </tr>
          </thead>
          <tbody>
            {logs.map((log) => (
              <tr key={log.id} className="border-b hover:bg-muted/30">
                <td className="px-3 py-2 whitespace-nowrap">
                  <div className="relative inline-flex items-center gap-1">
                    {log.other ? (
                      <div
                        className="relative"
                        onMouseEnter={() => setHoveredOtherId(log.id)}
                        onMouseLeave={() => setHoveredOtherId((current) => (current === log.id ? null : current))}
                      >
                        <button
                          type="button"
                          className="text-muted-foreground hover:text-foreground"
                          aria-label="Show other"
                        >
                          <ChevronRight className="h-3.5 w-3.5" />
                        </button>
                        {hoveredOtherId === log.id && (
                          <div className="absolute left-4 top-5 z-20 w-96 rounded-md border bg-background p-3 shadow-lg">
                            <pre className="whitespace-pre-wrap break-all text-[11px] text-muted-foreground">
                              {log.other}
                            </pre>
                          </div>
                        )}
                      </div>
                    ) : null}
                    <div>{new Date(log.created_at * 1000).toLocaleString()}</div>
                  </div>
                </td>
                <td className="px-3 py-2">
                  <div>{log.channel_name}</div>
                </td>
                <td className="px-3 py-2">
                  <div>{log.token_name || log.access_key_name || <span className="text-muted-foreground">-</span>}</div>
                </td>
                <td className="px-3 py-2 font-mono text-xs">
                  <div>
                    {log.requested_model === "auto"
                      ? `(auto)${log.model}`
                      : log.model}
                  </div>
                </td>
                <td className="px-3 py-2 whitespace-nowrap">
                  <div>{`${log.use_time || Math.ceil(log.latency_ms / 1000)}s${log.is_stream && log.first_token_ms > 0 ? ` / ${(log.first_token_ms / 1000).toFixed(1)}s` : ""}  ${log.is_stream ? t("log.streamShort") : t("log.nonStreamShort")}`}</div>
                </td>
                <td className="px-3 py-2 text-right">{log.prompt_tokens}</td>
                <td className="px-3 py-2 text-right">{log.completion_tokens}</td>
                <td className="px-3 py-2 min-w-64">
                  <div className="space-y-1 text-xs">
                    <div className="flex flex-wrap gap-x-3 gap-y-1 text-muted-foreground">
                      <span className={log.success ? "text-green-600" : "text-red-500"}>
                        {log.success ? t("log.success") : t("log.failed")}
                      </span>
                      <span>HTTP {log.status_code}</span>
                      {log.request_id ? <span>RID: {log.request_id}</span> : null}
                      {log.ip ? <span>IP: {log.ip}</span> : null}
                    </div>
                    {log.content ? (
                      <div className="whitespace-pre-wrap break-all">{log.content}</div>
                    ) : null}
                    {log.error_message ? (
                      <div className="text-red-500 whitespace-pre-wrap break-all">
                        {log.error_message}
                      </div>
                    ) : null}
                    {!log.content && !log.error_message ? (
                      <span className="text-muted-foreground">{t("log.noError")}</span>
                    ) : null}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {!logs.length && !isLoading && (
        <div className="flex h-32 items-center justify-center text-muted-foreground">
          {t("common.noData")}
        </div>
      )}
    </div>
  );
}
