/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import { fail } from '#utils/response';

export default function zodValidationHook(result, c) {
  if (!result.success) {
    return fail(c, result.error.flatten().fieldErrors, 400);
  }
}
