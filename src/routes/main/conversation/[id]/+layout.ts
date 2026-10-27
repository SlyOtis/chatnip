import type {LayoutLoad} from './$types';

export const load: LayoutLoad = async ({parent, params}) => {
  await parent();
  const conversationId = params.id;
  return {conversationId};
};