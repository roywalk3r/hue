/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import exploreExtract from './explore.extract.js';
import { axiosInstance } from '#services/axiosInstance';
import createEndpoint from '#utils/createEndpoint';
import { NotFoundError, validationError } from '#utils/errors';

export default async function exploreHandler(c) {
  const { query } = c.req.valid('param');
  const { page } = c.req.valid('query');

  const endpoint = createEndpoint(query, page);

  console.log(endpoint);

  const result = await axiosInstance(endpoint);

  if (!result.success) {
    throw new validationError('make sure given endpoint is correct');
  }
  const response = exploreExtract(result.data);

  if (response.response.length < 1) throw new NotFoundError();
  return response;
}
