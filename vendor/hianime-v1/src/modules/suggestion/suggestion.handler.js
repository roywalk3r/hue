/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import { validationError } from '#utils/errors';
import config from '#config/config';
import suggestionExtract from './suggestion.extract.js';

export default async function suggestionHandler(c) {
  const { keyword } = c.req.valid('query');

  const endpoint = `/ajax/search/suggest?keyword=${keyword}`;
  const Referer = `${config.baseurl}/home`;
  const res = await fetch(config.baseurl + endpoint, {
    headers: {
      Referer,
      ...config.headers,
    },
  });

  const data = await res.json();
  if (!data.status) throw new validationError('suggestion not found');

  const response = suggestionExtract(data.html);

  return response;
}
