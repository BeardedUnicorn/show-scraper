import { z } from "zod";

export const bucketKeys = [
  "DAY_OF",
  "LT_1W",
  "LT_2W",
  "LT_1M",
  "LT_2M",
  "GTE_2M",
] as const;

export const EventSchema = z.object({
  id: z.string(),
  source: z.string(),
  venue_id: z.string(),
  venue_name: z.string().nullable().optional(),
  venue_url: z.string().nullable().optional(),
  start_local: z.string().nullable().optional(),
  start_utc: z.string(),
  doors_local: z.string().nullable().optional(),
  artists: z.array(z.string()),
  is_all_ages: z.boolean().nullable().optional(),
  ticket_url: z.string().nullable().optional(),
  event_url: z.string().nullable().optional(),
  price_min_cents: z.number().nullable().optional(),
  price_max_cents: z.number().nullable().optional(),
  currency: z.string().nullable().optional(),
  tags: z.array(z.string()),
  scraped_at_utc: z.string(),
  extra: z.any(),
});

export const PendingEntrySchema = z.object({
  days_until: z.number(),
  event: EventSchema,
});

export const PendingBucketsSchema = z.record(
  z.string(),
  z.array(PendingEntrySchema)
);

export const VenueSchema = z.object({
  id: z.string(),
  name: z.string(),
  url: z.string().url(),
});

export const VenuesSchema = z.array(VenueSchema);

export type Event = z.infer<typeof EventSchema>;
export type PendingEntry = z.infer<typeof PendingEntrySchema>;
export type BucketKey = (typeof bucketKeys)[number];
export type PendingBuckets = Record<BucketKey, PendingEntry[]>;
export type Venue = z.infer<typeof VenueSchema>;
export type AppSettings = {
  llmModel: string;
  llmEndpoint: string;
  dataDirectory: string;
  autoOpenPreview: boolean;
  notifyOnPost: boolean;
};

export function createEmptyBuckets(): PendingBuckets {
  return bucketKeys.reduce((acc, key) => {
    acc[key] = [];
    return acc;
  }, {} as PendingBuckets);
}

export function normalizeBuckets(input: unknown): PendingBuckets {
  const parsed = PendingBucketsSchema.parse(input);
  const result = createEmptyBuckets();
  for (const key of Object.keys(parsed)) {
    if ((bucketKeys as readonly string[]).includes(key)) {
      result[key as BucketKey] = parsed[key];
    }
  }
  return result;
}

export function parseVenues(input: unknown): Venue[] {
  return VenuesSchema.parse(input);
}
