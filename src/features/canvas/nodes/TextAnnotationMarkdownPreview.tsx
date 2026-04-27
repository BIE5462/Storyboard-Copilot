import { useCallback } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkBreaks from 'remark-breaks';
import remarkGfm from 'remark-gfm';

import { openExternalUrl } from '@/features/platform/platformService';

interface TextAnnotationMarkdownPreviewProps {
  content: string;
}

export function TextAnnotationMarkdownPreview({ content }: TextAnnotationMarkdownPreviewProps) {
  const handleMarkdownLinkClick = useCallback((href?: string) => {
    if (!href) {
      return;
    }
    void openExternalUrl(href);
  }, []);

  return (
    <div className="markdown-body break-words [&_a]:text-accent [&_blockquote]:border-l-2 [&_blockquote]:border-white/20 [&_blockquote]:pl-3 [&_code]:rounded [&_code]:bg-white/10 [&_code]:px-1 [&_code]:py-0.5 [&_h1]:text-base [&_h1]:font-semibold [&_h2]:text-[15px] [&_h2]:font-semibold [&_h3]:text-sm [&_h3]:font-semibold [&_hr]:border-white/10 [&_li]:my-0.5 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-0 [&_p+_p]:mt-4 [&_pre]:overflow-auto [&_pre]:rounded-md [&_pre]:bg-black/30 [&_pre]:p-2 [&_table]:w-full [&_table]:border-collapse [&_table]:text-xs [&_td]:border [&_td]:border-white/10 [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-white/10 [&_th]:px-2 [&_th]:py-1 [&_ul]:list-disc [&_ul]:pl-5">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkBreaks]}
        components={{
          a: ({ href, children, ...props }) => (
            <a
              {...props}
              href={href}
              target="_blank"
              rel="noreferrer"
              onClick={(event) => {
                event.preventDefault();
                handleMarkdownLinkClick(href);
              }}
            >
              {children}
            </a>
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
