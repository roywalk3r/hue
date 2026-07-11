/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import { load } from 'cheerio';

export default function nextEpScheduleExtract(html) {
  const $ = load(html);

  const time = $('.schedule-alert #schedule-date').attr('data-value') || null;

  return { time };
}
