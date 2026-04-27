/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import { Redis } from '@upstash/redis';

export default async function connectRedis() {
  const isRedisEnv = Boolean(
    process.env.UPSTASH_REDIS_REST_URL && process.env.UPSTASH_REDIS_REST_TOKEN
  );

  if (!isRedisEnv)
    return {
      exist: false,
      redis: null,
    };
  const redis = Redis.fromEnv();

  return {
    exist: true,
    redis,
  };
}
