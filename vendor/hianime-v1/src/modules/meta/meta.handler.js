/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import { genres, azList, exploreRoutes, filterOptions } from '#config/meta';

export default async function metaHandler() {
  return {
    genres,
    azList,
    exploreRoutes,
    filterOptions: { ...filterOptions, genres },
  };
}
