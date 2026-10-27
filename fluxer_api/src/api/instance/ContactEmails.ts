// SPDX-License-Identifier: AGPL-3.0-or-later

import {Config} from '@app/api/Config';

export interface ContactEmails {
	appealsEmail: string | null;
	safetyEmail: string | null;
	supportEmail: string | null;
}

const HOSTED_CONTACT_EMAILS: ContactEmails = {
	appealsEmail: 'appeals@nekochat-site.example',
	safetyEmail: 'safety@nekochat-site.example',
	supportEmail: 'support@nekochat-site.example',
};

const SELF_HOSTED_CONTACT_EMAILS: ContactEmails = {
	appealsEmail: null,
	safetyEmail: null,
	supportEmail: null,
};

export function resolveContactEmails(): ContactEmails {
	return Config.instance.selfHosted ? SELF_HOSTED_CONTACT_EMAILS : HOSTED_CONTACT_EMAILS;
}
