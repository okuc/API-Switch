import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  getDashboardStats,
  getModelConsumption,
  getCallTrend,
  getModelDistribution,
  getModelRanking,
  getUserRanking,
  getUserTrend,
} from "@/lib/api";
import type { DashboardFilter } from "@/types";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  LineChart,
  Line,
} from "recharts";

const COLORS = [
  "#8884d8", "#82ca9d", "#ffc658", "#ff7300", "#0088fe",
  "#00C49F", "#FFBB28", "#FF8042", "#a855f7", "#ec4899",
];

type SeriesPoint = {
  time: string;
  [key: string]: string | number;
};

function buildSeriesData(
  items: Array<{ time: string; model: string; value: number }> | undefined,
  topN = 8,
): { data: SeriesPoint[]; series: string[] } {
  if (!items?.length) {
    return { data: [], series: [] };
  }

  const totals = new Map<string, number>();
  for (const item of items) {
    totals.set(item.model, (totals.get(item.model) ?? 0) + item.value);
  }

  const series = [...totals.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, topN)
    .map(([model]) => model);

  const allowed = new Set(series);
  const byTime = new Map<string, SeriesPoint>();

  for (const item of items) {
    const timeEntry = byTime.get(item.time) ?? { time: item.time };
    const key = allowed.has(item.model) ? item.model : "Other";
    const current = typeof timeEntry[key] === "number" ? Number(timeEntry[key]) : 0;
    timeEntry[key] = current + item.value;
    byTime.set(item.time, timeEntry);
  }

  const finalSeries = byTime.size && items.some((item) => !allowed.has(item.model))
    ? [...series, "Other"]
    : series;

  return {
    data: [...byTime.values()].sort((a, b) => String(a.time).localeCompare(String(b.time))),
    series: finalSeries,
  };
}

function StatCard({ title, value, sub }: { title: string; value: string | number; sub?: string }) {
  return (
    <Card>
      <CardContent className="p-4">
        <p className="text-sm text-muted-foreground">{title}</p>
        <p className="text-2xl font-bold mt-1">{value}</p>
        {sub && <p className="text-xs text-muted-foreground mt-1">{sub}</p>}
      </CardContent>
    </Card>
  );
}

function TopListCard({
  title,
  items,
  valueLabel,
  emptyText,
}: {
  title: string;
  items: Array<{ name: string; count: number; extra?: string }>;
  valueLabel: string;
  emptyText: string;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {items.length ? (
          items.map((item, index) => (
            <div key={`${title}-${item.name}-${index}`} className="rounded-md border p-3">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-xs text-muted-foreground">#{index + 1}</div>
                  <div className="font-medium truncate">{item.name}</div>
                  {item.extra ? (
                    <div className="text-xs text-muted-foreground truncate">{item.extra}</div>
                  ) : null}
                </div>
                <div className="text-right text-sm">
                  <div>{item.count} {valueLabel}</div>
                </div>
              </div>
            </div>
          ))
        ) : (
          <div className="text-sm text-muted-foreground">{emptyText}</div>
        )}
      </CardContent>
    </Card>
  );
}

export function DashboardPage() {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<DashboardFilter>({ granularity: "hour" });

  const { data: stats } = useQuery({
    queryKey: ["dashboardStats", filter],
    queryFn: () => getDashboardStats(filter),
  });

  const { data: consumption } = useQuery({
    queryKey: ["modelConsumption", filter],
    queryFn: () => getModelConsumption(filter),
  });

  const { data: callTrend } = useQuery({
    queryKey: ["callTrend", filter],
    queryFn: () => getCallTrend(filter),
  });

  const { data: distribution } = useQuery({
    queryKey: ["modelDistribution", filter],
    queryFn: () => getModelDistribution(filter),
  });

  const { data: ranking } = useQuery({
    queryKey: ["modelRanking", filter],
    queryFn: () => getModelRanking(filter),
  });

  const { data: userRanking } = useQuery({
    queryKey: ["userRanking", filter],
    queryFn: () => getUserRanking(filter),
  });

  const { data: userTrend } = useQuery({
    queryKey: ["userTrend", filter],
    queryFn: () => getUserTrend(filter),
  });

  const totalTokens = (stats?.total_prompt_tokens ?? 0) + (stats?.total_completion_tokens ?? 0);
  const todayTokens = (stats?.today_prompt_tokens ?? 0) + (stats?.today_completion_tokens ?? 0);
  const topModels = (ranking || []).slice(0, 5);
  const topUsers = (userRanking || []).slice(0, 5);
  const consumptionSeries = buildSeriesData(consumption);
  const callTrendSeries = buildSeriesData(callTrend);
  const userTrendSeries = buildSeriesData(userTrend, 6);

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
    setFilter((prev) => ({ ...prev, start_time: start || undefined, end_time: undefined }));
  };

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("dashboard.title")}</h1>
        <div className="flex gap-2">
          {["today", "7d", "30d"].map((range) => (
            <Button key={range} variant="outline" size="sm" onClick={() => setTimeRange(range)}>
              {t(`dashboard.filter.${range === "today" ? "today" : range === "7d" ? "sevenDays" : "thirtyDays"}`)}
            </Button>
          ))}
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-2 xl:grid-cols-4 gap-4 mb-6">
        <StatCard
          title={t("dashboard.cards.todayRequests")}
          value={stats?.today_requests ?? 0}
          sub={`${t("dashboard.total")}: ${stats?.total_requests ?? 0}`}
        />
        <StatCard
          title={t("dashboard.cards.todayTokens")}
          value={todayTokens}
          sub={`${t("dashboard.prompt")}: ${stats?.today_prompt_tokens ?? 0} / ${t("dashboard.completion")}: ${stats?.today_completion_tokens ?? 0}`}
        />
        <StatCard
          title={t("dashboard.cards.performance")}
          value={`${stats?.rpm ?? 0} RPM`}
          sub={`${stats?.tpm ?? 0} TPM`}
        />
        <StatCard
          title={t("dashboard.cards.successRate")}
          value={`${((stats?.success_rate ?? 0) * 100).toFixed(1)}%`}
          sub={`${t("dashboard.avgLatency")}: ${stats?.avg_latency_ms ?? 0}ms`}
        />
        <StatCard
          title={t("dashboard.cards.totalTokens")}
          value={totalTokens}
          sub={`${t("dashboard.total")}: ${totalTokens}`}
        />
        <StatCard
          title={t("dashboard.cards.totalPrompt")}
          value={stats?.total_prompt_tokens ?? 0}
          sub={`${t("dashboard.today")}: ${stats?.today_prompt_tokens ?? 0}`}
        />
        <StatCard
          title={t("dashboard.cards.totalCompletion")}
          value={stats?.total_completion_tokens ?? 0}
          sub={`${t("dashboard.today")}: ${stats?.today_completion_tokens ?? 0}`}
        />
        <StatCard
          title={t("dashboard.cards.avgLatency")}
          value={`${Math.round(stats?.avg_latency_ms ?? 0)} ms`}
          sub={`${t("dashboard.successRate")}: ${((stats?.success_rate ?? 0) * 100).toFixed(1)}%`}
        />
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
        {/* Charts */}
        <Tabs defaultValue="consumption">
          <TabsList>
            <TabsTrigger value="consumption">{t("dashboard.charts.consumption")}</TabsTrigger>
            <TabsTrigger value="callTrend">{t("dashboard.charts.callTrend")}</TabsTrigger>
            <TabsTrigger value="distribution">{t("dashboard.charts.distribution")}</TabsTrigger>
            <TabsTrigger value="ranking">{t("dashboard.charts.ranking")}</TabsTrigger>
            <TabsTrigger value="userRanking">{t("dashboard.charts.userRanking")}</TabsTrigger>
            <TabsTrigger value="userTrend">{t("dashboard.charts.userTrend")}</TabsTrigger>
          </TabsList>

          <TabsContent value="consumption">
            <Card>
              <CardHeader className="pb-0">
                <div className="flex items-center justify-between gap-3">
                  <CardTitle>{t("dashboard.charts.consumption")}</CardTitle>
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <span>{t("dashboard.filter.hour")}</span>
                    <Switch
                      checked={filter.granularity === "day"}
                      onCheckedChange={(checked) =>
                        setFilter((prev) => ({
                          ...prev,
                          granularity: checked ? "day" : "hour",
                        }))
                      }
                    />
                    <span>{t("dashboard.filter.day")}</span>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <BarChart data={consumptionSeries.data}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    {consumptionSeries.series.map((series, index) => (
                      <Bar
                        key={series}
                        dataKey={series}
                        stackId="consumption"
                        fill={COLORS[index % COLORS.length]}
                      />
                    ))}
                  </BarChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="callTrend">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <LineChart data={callTrendSeries.data}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    {callTrendSeries.series.map((series, index) => (
                      <Line
                        key={series}
                        type="monotone"
                        dataKey={series}
                        stroke={COLORS[index % COLORS.length]}
                        strokeWidth={2}
                        dot={false}
                      />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="distribution">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <PieChart>
                    <Pie
                      data={distribution || []}
                      dataKey="count"
                      nameKey="model"
                      cx="50%"
                      cy="50%"
                      outerRadius={150}
                      label
                    >
                      {(distribution || []).map((_, index) => (
                        <Cell key={index} fill={COLORS[index % COLORS.length]} />
                      ))}
                    </Pie>
                    <Tooltip />
                    <Legend />
                  </PieChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="ranking">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <BarChart data={(ranking || []).slice(0, 20)} layout="vertical">
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis type="number" />
                    <YAxis dataKey="model" type="category" width={150} />
                    <Tooltip />
                    <Bar dataKey="count" fill="#82ca9d" />
                  </BarChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="userRanking">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <BarChart data={(userRanking || []).slice(0, 20)} layout="vertical">
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis type="number" />
                    <YAxis dataKey="access_key_name" type="category" width={150} />
                    <Tooltip />
                    <Bar dataKey="count" fill="#f59e0b" />
                  </BarChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="userTrend">
            <Card>
              <CardContent className="pt-6">
                <ResponsiveContainer width="100%" height={400}>
                  <LineChart data={userTrendSeries.data}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    {userTrendSeries.series.map((series, index) => (
                      <Line
                        key={series}
                        type="monotone"
                        dataKey={series}
                        stroke={COLORS[index % COLORS.length]}
                        strokeWidth={2}
                        dot={false}
                      />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>

        <div className="space-y-6">
          <TopListCard
            title={t("dashboard.topModels")}
            valueLabel={t("dashboard.requests")}
            emptyText={t("common.noData")}
            items={topModels.map((item) => ({
              name: item.model,
              count: item.count,
              extra: `${item.prompt_tokens + item.completion_tokens} tokens`,
            }))}
          />

          <TopListCard
            title={t("dashboard.topUsers")}
            valueLabel={t("dashboard.requests")}
            emptyText={t("common.noData")}
            items={topUsers.map((item) => ({
              name: item.access_key_name,
              count: item.count,
              extra: `${item.prompt_tokens + item.completion_tokens} tokens`,
            }))}
          />
        </div>
      </div>
    </div>
  );
}
