import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  BucketKey,
  PendingBuckets,
  createEmptyBuckets,
  normalizeBuckets,
} from "../../lib/types";

type PendingState = {
  data: PendingBuckets;
  loading: boolean;
  refresh: () => Promise<void>;
};

export function usePendingBuckets(): PendingState {
  const [data, setData] = useState<PendingBuckets>(createEmptyBuckets);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const response = await invoke("list_pending_buckets");
      setData(normalizeBuckets(response));
    } finally {
      setLoading(false);
    }
  }, []);

  return useMemo(
    () => ({
      data,
      loading,
      refresh,
    }),
    [data, loading, refresh]
  );
}

export function flattenBuckets(buckets: PendingBuckets) {
  return (Object.keys(buckets) as BucketKey[]).flatMap((key) => buckets[key]);
}
