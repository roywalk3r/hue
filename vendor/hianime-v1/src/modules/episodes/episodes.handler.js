/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import config from '#config/config';
import episodesExtract from './episodes.extract.js';
import { NotFoundError } from '#utils/errors';

export default async function episodesHandler(c) {
  const { id } = c.req.valid('param');

  const Referer = `/watch/${id}`;
  const idNum = id.split('-').at(-1);
  const ajaxUrl = `/ajax/v2/episode/list/${idNum}`;

  try {
    const res = await fetch(config.baseurl + ajaxUrl, {
      headers: {
        Referer: Referer,
        ...config.headers,
      },
    });

    const data = await res.json();
    const response = episodesExtract(data.html);
    return response;
  } catch (err) {
    console.log(err.message);
    throw new NotFoundError('episodes Not Found');
  }
}
