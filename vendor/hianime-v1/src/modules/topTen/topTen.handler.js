/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import { axiosInstance } from '#services/axiosInstance';
import { validationError } from '#utils/errors';
import topTenExtract from './topTen.extract.js';

export default async function topTenHandler() {
  const result = await axiosInstance('/home');
  if (!result.success) {
    throw new validationError(result.message);
  }
  const response = topTenExtract(result.data);
  return response;
}
