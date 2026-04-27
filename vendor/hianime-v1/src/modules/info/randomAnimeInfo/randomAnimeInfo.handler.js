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
import infoExtract from '../info.extract.js';

export default async function randomAnimeInfoHandler() {
  const result = await axiosInstance(`/random`);
  if (!result.success) {
    throw new validationError(result.message, 'something went wrong');
  }
  const response = infoExtract(result.data);
  return response;
}
