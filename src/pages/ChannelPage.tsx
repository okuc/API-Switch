import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Edit, Plus, RefreshCw, Save, TestTube2, Trash2, Link2, CheckSquare, Square, Import } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  listChannels,
  createChannel,
  updateChannel,
  deleteChannel,
  fetchModels,
  selectModels,
} from "@/lib/api";
import { API_TYPE_OPTIONS, API_TYPE_DEFAULT_URLS } from "@/types";
import type { ApiType, Channel, CreateChannelParams, ModelInfo, UpdateChannelParams } from "@/types";

type ChannelFormState = {
  id?: string;
  name: string;
  api_type: ApiType;
  base_url: string;
  api_key: string;
  notes: string;
  enabled: boolean;
};

const defaultChannelForm = (): ChannelFormState => ({
  name: "",
  api_type: "openai",
  base_url: API_TYPE_DEFAULT_URLS.openai,
  api_key: "",
  notes: "",
  enabled: true,
});

export function ChannelPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [keyword, setKeyword] = useState("");
  const [showEdit, setShowEdit] = useState(false);
  const [editingChannel, setEditingChannel] = useState<Channel | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const { data: channels, isLoading } = useQuery({
    queryKey: ["channels"],
    queryFn: listChannels,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteChannel,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["channels"] }),
    onError: (err) => {
      alert(`Delete failed: ${err}`);
    },
  });

  const filteredChannels = useMemo(() => {
    if (!channels) return [];
    const term = keyword.trim().toLowerCase();
    if (!term) return channels;
    return channels.filter((channel) => {
      const haystack = [channel.name, channel.api_type, channel.base_url, channel.notes]
        .join(" ")
        .toLowerCase();
      return haystack.includes(term);
    });
  }, [channels, keyword]);

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between gap-4 flex-wrap">
        <div>
          <h1 className="text-xl font-semibold">{t("channel.title")}</h1>
          <p className="text-sm text-muted-foreground mt-1">{t("channel.description")}</p>
        </div>
        <div className="flex items-center gap-3">
          <Input
            className="w-72"
            placeholder={t("channel.search")}
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
          />
          <Button
            size="sm"
            className="gap-1.5"
            onClick={() => {
              setEditingChannel(null);
              setShowEdit(true);
            }}
          >
            <Plus className="h-4 w-4" />
            {t("channel.add")}
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle>{t("channel.listTitle")}</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <div className="overflow-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-muted/50">
                  <th className="px-4 py-3 text-left font-medium">{t("channel.name")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.type")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.baseUrl")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.status")}</th>
                  <th className="px-4 py-3 text-left font-medium">{t("channel.modelCount")}</th>
                  <th className="px-4 py-3 text-right font-medium">{t("channel.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {filteredChannels.map((channel) => (
                  <ChannelRow
                    key={channel.id}
                    channel={channel}
                    expanded={expandedId === channel.id}
                    onToggleExpand={() =>
                      setExpandedId((current) => (current === channel.id ? null : channel.id))
                    }
                    onEdit={() => {
                      setEditingChannel(channel);
                      setShowEdit(true);
                    }}
                    onDelete={() => deleteMutation.mutate(channel.id)}
                  />
                ))}
              </tbody>
            </table>
          </div>

          {!filteredChannels.length && (
            <div className="flex h-48 items-center justify-center text-muted-foreground">
              {t("common.noData")}
            </div>
          )}
        </CardContent>
      </Card>

      <ChannelEditorDialog
        open={showEdit}
        channel={editingChannel}
        onOpenChange={setShowEdit}
      />
    </div>
  );
}

function ChannelRow({
  channel,
  expanded,
  onToggleExpand,
  onEdit,
  onDelete,
}: {
  channel: Channel;
  expanded: boolean;
  onToggleExpand: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [modelSearch, setModelSearch] = useState("");
  const [fetching, setFetching] = useState(false);

  const fetchMutation = useMutation({
    mutationFn: () => fetchModels(channel.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      setFetching(false);
    },
    onError: (err) => {
      setFetching(false);
      alert(`Fetch models failed: ${err}`);
    },
  });

  const testMutation = useMutation({
    mutationFn: async () => {
      const models = await fetchModels(channel.id);
      return models;
    },
    onSuccess: (models) => {
      alert(`Test success: fetched ${models.length} models`);
    },
    onError: (err) => {
      alert(`Test failed: ${err}`);
    },
  });

  const selectMutation = useMutation({
    mutationFn: (models: string[]) => selectModels(channel.id, models),
    onMutate: async (newSelected) => {
      await queryClient.cancelQueries({ queryKey: ["channels"] });
      const previous = queryClient.getQueryData<Channel[]>(["channels"]);
      queryClient.setQueryData<Channel[]>(["channels"], (old) =>
        old?.map((c) =>
          c.id === channel.id ? { ...c, selected_models: newSelected } : c,
        ),
      );
      return { previous };
    },
    onError: (err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(["channels"], context.previous);
      }
      alert(`Select models failed: ${err}`);
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["channels"] }),
  });

  const availableModels: ModelInfo[] = channel.available_models || [];
  const selectedModels: string[] = channel.selected_models || [];

  const filteredModels = modelSearch
    ? availableModels.filter((m) =>
        m.name.toLowerCase().includes(modelSearch.toLowerCase()),
      )
    : availableModels;

  const toggleModel = (modelName: string) => {
    if (selectMutation.isPending) return;
    const newSelected = selectedModels.includes(modelName)
      ? selectedModels.filter((m) => m !== modelName)
      : [...selectedModels, modelName];
    selectMutation.mutate(newSelected);
  };

  const selectAllFiltered = () => {
    if (selectMutation.isPending) return;
    const merged = Array.from(new Set([...selectedModels, ...filteredModels.map((m) => m.name)]));
    selectMutation.mutate(merged);
  };

  const clearAllSelected = () => {
    if (selectMutation.isPending) return;
    selectMutation.mutate([]);
  };

  return (
    <>
      <tr className="border-b hover:bg-muted/30">
        <td className="px-4 py-3">
          <button type="button" className="text-left" onClick={onToggleExpand}>
            <div className="font-medium">{channel.name}</div>
            {channel.notes ? (
              <div className="text-xs text-muted-foreground mt-1 line-clamp-1">{channel.notes}</div>
            ) : null}
          </button>
        </td>
        <td className="px-4 py-3">
          <span className="inline-flex rounded bg-secondary px-2 py-0.5 text-xs text-muted-foreground">
            {channel.api_type}
          </span>
        </td>
        <td className="px-4 py-3 font-mono text-xs max-w-[320px] truncate">{channel.base_url}</td>
        <td className="px-4 py-3">
          <span className={cn("text-xs", channel.enabled ? "text-green-600" : "text-muted-foreground") }>
            {channel.enabled ? t("channel.enabled") : t("channel.disabled")}
          </span>
        </td>
        <td className="px-4 py-3">{selectedModels.length}</td>
        <td className="px-4 py-3">
          <div className="flex items-center justify-end gap-1">
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onEdit}>
              <Edit className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={selectAllFiltered}>
              <Import className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => testMutation.mutate()}
            >
              <TestTube2 className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onDelete}>
              <Trash2 className="h-4 w-4 text-destructive" />
            </Button>
          </div>
        </td>
      </tr>

      {expanded && (
        <tr className="border-b bg-muted/10">
          <td colSpan={6} className="px-4 py-4">
            <div className="space-y-4">
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5"
                  onClick={() => {
                    setFetching(true);
                    fetchMutation.mutate();
                  }}
                  disabled={fetching}
                >
                  <RefreshCw className={cn("h-3.5 w-3.5", fetching && "animate-spin")} />
                  {fetching ? t("channel.models.fetching") : t("channel.models.fetch")}
                </Button>
                <Button size="sm" variant="outline" className="gap-1.5" onClick={onEdit}>
                  <Edit className="h-3.5 w-3.5" />
                  {t("channel.edit")}
                </Button>
              </div>

              <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4 text-sm">
                <InfoBlock label={t("channel.baseUrl")} value={channel.base_url} mono />
                <InfoBlock label={t("channel.apiKey")} value={maskSecret(channel.api_key)} mono />
                <InfoBlock label={t("channel.updatedAt")} value={new Date(channel.updated_at * 1000).toLocaleString()} />
                <InfoBlock label={t("channel.modelCount")} value={`${selectedModels.length} / ${availableModels.length}`} />
              </div>

              <div className="rounded-md border bg-muted/20 p-4 space-y-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Link2 className="h-4 w-4" />
                  {t("channel.poolSyncTitle")}
                </div>
                <p className="text-sm text-muted-foreground">
                  {t("channel.poolSyncDesc")}
                </p>
                {selectedModels.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {selectedModels.slice(0, 12).map((model) => (
                      <span key={model} className="rounded-full border bg-background px-2.5 py-1 text-xs">
                        {model}
                      </span>
                    ))}
                    {selectedModels.length > 12 ? (
                      <span className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground">
                        +{selectedModels.length - 12}
                      </span>
                    ) : null}
                  </div>
                ) : (
                  <p className="text-xs text-muted-foreground">{t("channel.poolSyncEmpty")}</p>
                )}
              </div>

              <div className="space-y-2">
                <div className="flex flex-wrap gap-2 items-center">
                  <Input
                    placeholder={t("channel.models.search")}
                    value={modelSearch}
                    onChange={(e) => setModelSearch(e.target.value)}
                    className="h-8 text-sm flex-1 min-w-64"
                  />
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={selectAllFiltered}>
                    <CheckSquare className="h-3.5 w-3.5" />
                    {t("channel.models.selectFiltered")}
                  </Button>
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={clearAllSelected}>
                    <Square className="h-3.5 w-3.5" />
                    {t("channel.models.clearSelected")}
                  </Button>
                </div>
                {availableModels.length > 0 ? (
                  <div className="max-h-72 overflow-y-auto rounded-md border bg-background">
                    {filteredModels.map((model) => (
                      <label
                        key={model.id}
                        className="flex items-center gap-2 px-3 py-2 border-b last:border-b-0 hover:bg-accent cursor-pointer text-sm"
                      >
                        <Checkbox
                          checked={selectedModels.includes(model.name)}
                          onCheckedChange={() => toggleModel(model.name)}
                        />
                        <span className="truncate">{model.name}</span>
                        {model.owned_by ? (
                          <span className="text-xs text-muted-foreground ml-auto">{model.owned_by}</span>
                        ) : null}
                      </label>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground py-3">{t("channel.models.empty")}</p>
                )}
              </div>
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

function ChannelEditorDialog({
  open,
  channel,
  onOpenChange,
}: {
  open: boolean;
  channel: Channel | null;
  onOpenChange: (value: boolean) => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState<ChannelFormState>(defaultChannelForm());

  useEffect(() => {
    if (!open) return;
    if (channel) {
      setForm({
        id: channel.id,
        name: channel.name,
        api_type: channel.api_type as ApiType,
        base_url: channel.base_url,
        api_key: channel.api_key,
        notes: channel.notes,
        enabled: channel.enabled,
      });
    } else {
      setForm(defaultChannelForm());
    }
  }, [channel, open]);

  const saveMutation = useMutation({
    mutationFn: async (values: ChannelFormState) => {
      if (values.id) {
        const payload: UpdateChannelParams = {
          id: values.id,
          name: values.name,
          api_type: values.api_type,
          base_url: values.base_url,
          api_key: values.api_key,
          notes: values.notes,
          enabled: values.enabled,
        };
        return updateChannel(payload);
      }
      const payload: CreateChannelParams = {
        name: values.name,
        api_type: values.api_type,
        base_url: values.base_url,
        api_key: values.api_key,
        notes: values.notes,
      };
      return createChannel(payload);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      onOpenChange(false);
    },
    onError: (err) => {
      alert(`Save failed: ${err}`);
    },
  });

  const setValue = <K extends keyof ChannelFormState>(key: K, value: ChannelFormState[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const testMutation = useMutation({
    mutationFn: async () => {
      if (!channel?.id) {
        throw new Error("Save channel before testing");
      }
      const models = await fetchModels(channel.id);
      return models;
    },
    onSuccess: (models) => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
      alert(`Test success: fetched ${models.length} models`);
    },
    onError: (err) => {
      alert(`Test failed: ${err}`);
    },
  });

  const handleApiTypeChange = (type: ApiType) => {
    setForm((prev) => ({
      ...prev,
      api_type: type,
      base_url: prev.base_url === API_TYPE_DEFAULT_URLS[prev.api_type]
        ? API_TYPE_DEFAULT_URLS[type] || prev.base_url
        : prev.base_url,
    }));
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>{channel ? t("channel.edit") : t("channel.add")}</DialogTitle>
        </DialogHeader>

        <div className="grid gap-6 md:grid-cols-2">
          <div className="space-y-4">
            <div className="space-y-2">
              <Label>{t("channel.name")}</Label>
              <Input value={form.name} onChange={(e) => setValue("name", e.target.value)} />
            </div>

            <div className="space-y-2">
              <Label>{t("channel.type")}</Label>
              <Select value={form.api_type} onValueChange={(v) => handleApiTypeChange(v as ApiType)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {API_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label>{t("channel.baseUrl")}</Label>
              <Input value={form.base_url} onChange={(e) => setValue("base_url", e.target.value)} />
              <div className="flex flex-wrap gap-2 pt-1">
                {API_TYPE_OPTIONS.map((option) => {
                  const preset = API_TYPE_DEFAULT_URLS[option.value];
                  if (!preset) return null;
                  return (
                    <Button
                      key={option.value}
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => setValue("base_url", preset)}
                    >
                      {option.label}
                    </Button>
                  );
                })}
              </div>
            </div>

            <div className="space-y-2">
              <Label>{t("channel.apiKey")}</Label>
              <Input
                type="password"
                value={form.api_key}
                onChange={(e) => setValue("api_key", e.target.value)}
              />
            </div>
          </div>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label>{t("channel.notes")}</Label>
              <textarea
                value={form.notes}
                onChange={(e) => setValue("notes", e.target.value)}
                className="min-h-32 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none"
              />
            </div>

            <div className="rounded-md border p-4 space-y-3 bg-muted/20">
              <div className="font-medium text-sm">{t("channel.advanced")}</div>
              <div className="flex items-center justify-between rounded-md border bg-background px-3 py-2">
                <div>
                  <div className="text-sm font-medium">{t("channel.enabled")}</div>
                  <div className="text-xs text-muted-foreground">{t("channel.enabledDesc")}</div>
                </div>
                <Checkbox
                  checked={form.enabled}
                  onCheckedChange={(checked) => setValue("enabled", Boolean(checked))}
                />
              </div>
              {channel ? (
                <div className="flex items-center justify-between rounded-md border bg-background px-3 py-2">
                  <div>
                    <div className="text-sm font-medium">{t("common.test")}</div>
                    <div className="text-xs text-muted-foreground">{t("channel.testDesc")}</div>
                  </div>
                  <Button size="sm" variant="outline" className="gap-1.5" onClick={() => testMutation.mutate()}>
                    <TestTube2 className="h-3.5 w-3.5" />
                    {t("common.test")}
                  </Button>
                </div>
              ) : null}
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button
            className="gap-1.5"
            onClick={() => saveMutation.mutate(form)}
            disabled={!form.name || !form.base_url || !form.api_key || saveMutation.isPending}
          >
            <Save className="h-4 w-4" />
            {channel ? t("common.save") : t("common.add")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function InfoBlock({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-md border bg-background p-3">
      <div className="text-xs text-muted-foreground mb-1">{label}</div>
      <div className={cn("text-sm break-all", mono && "font-mono text-xs")}>{value}</div>
    </div>
  );
}

function maskSecret(value: string) {
  if (!value) return "-";
  if (value.length <= 8) return "*".repeat(value.length);
  return `${value.slice(0, 4)}****${value.slice(-4)}`;
}
